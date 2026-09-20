"""Browser scripts for the bounded WebGPU recovery runner."""

PAGE = """<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Metis WebGPU recovery</title></head>
<body>
  <canvas id="recovery" width="32" height="32"></canvas>
  <pre id="recovery-error" hidden></pre>
  <script type="module">
    import init from './canvas_recovery.js';
    try {
      window.recovery = await init();
      window.__recoveryReady = true;
    } catch (error) {
      window.__recoveryLoadError = String(error).slice(0, 512);
    }
  </script>
</body>
</html>
"""

WAIT_FOR_LOADER = """
const done = arguments[arguments.length - 1];
const limit = arguments[0];
let settled = false;
const finish = (value) => {
  if (settled) return;
  settled = true;
  window.clearTimeout(timer);
  done(value);
};
const inspect = () => {
  if (settled) return;
  if (window.__recoveryLoadError) {
    finish({ok: false, error: window.__recoveryLoadError});
    return;
  }
  if (window.__recoveryReady && window.recovery) {
    finish({ok: true});
    return;
  }
  window.requestAnimationFrame(inspect);
};
const timer = window.setTimeout(
  () => finish({ok: false, error: 'WASM loader deadline exceeded'}), limit
);
inspect();
"""

INSTALL_GPU_TRACE = """
const maxDevices = arguments[0];
const maxEvents = arguments[1];
if (!navigator.gpu || !globalThis.GPUAdapter || !globalThis.GPUCanvasContext ||
    !globalThis.GPUQueue) {
  return {ok: false, error: 'required WebGPU interfaces are unavailable'};
}
if (window.__gpuRecoveryTrace) {
  return {ok: false, error: 'WebGPU trace is already installed'};
}
const state = {
  adapter: null, devices: [], configurations: [], uploads: [], errors: [], overflow: false
};
const deviceObjects = [];
const queueOwners = new WeakMap();
const bounded = (value) => String(value ?? '').slice(0, 512);
const append = (items, value) => {
  if (items.length >= maxEvents) { state.overflow = true; return; }
  items.push(value);
};
const deviceId = (device) => {
  const index = deviceObjects.indexOf(device);
  return index < 0 ? null : index + 1;
};
const originalRequestDevice = GPUAdapter.prototype.requestDevice;
GPUAdapter.prototype.requestDevice = function(...args) {
  if (state.adapter === null) {
    const info = this.info || {};
    state.adapter = {
      vendor: bounded(info.vendor), architecture: bounded(info.architecture),
      device: bounded(info.device), description: bounded(info.description),
      is_fallback_adapter: this.isFallbackAdapter === true
    };
  }
  const promise = Reflect.apply(originalRequestDevice, this, args);
  return Promise.resolve(promise).then((device) => {
    if (deviceObjects.length >= maxDevices) {
      state.overflow = true;
      return device;
    }
    deviceObjects.push(device);
    const record = {id: deviceObjects.length, lost: null, uncaptured_errors: []};
    queueOwners.set(device.queue, record.id);
    state.devices.push(record);
    device.addEventListener('uncapturederror', (event) => {
      const error = event && event.error;
      const entry = {
        device_id: record.id,
        name: bounded(error && error.name),
        message: bounded(error && error.message)
      };
      append(record.uncaptured_errors, entry);
      append(state.errors, {kind: 'uncapturederror', ...entry});
    });
    Promise.resolve(device.lost).then((info) => {
      record.lost = {reason: bounded(info && info.reason), message: bounded(info && info.message)};
    }, (error) => {
      record.lost = {rejected: true, message: bounded(error)};
      append(state.errors, {kind: 'device-lost-rejection', message: bounded(error)});
    });
    return device;
  });
};
const originalConfigure = GPUCanvasContext.prototype.configure;
GPUCanvasContext.prototype.configure = function(configuration) {
  const result = Reflect.apply(originalConfigure, this, [configuration]);
  append(state.configurations, {
    device_id: deviceId(configuration && configuration.device),
    format: bounded(configuration && configuration.format),
    alpha_mode: bounded(configuration && configuration.alphaMode)
  });
  return result;
};
for (const method of ['writeTexture', 'copyExternalImageToTexture', 'writeBuffer']) {
  const original = GPUQueue.prototype[method];
  if (typeof original !== 'function') continue;
  GPUQueue.prototype[method] = function(...args) {
    const result = Reflect.apply(original, this, args);
    append(state.uploads, {method, device_id: queueOwners.get(this) ?? null});
    return result;
  };
}
const recordWindowError = (kind, value) => append(
  state.errors, {kind, message: bounded(value && (value.message ?? value.reason ?? value))}
);
window.addEventListener('error', (event) => recordWindowError('window-error', event));
window.addEventListener('unhandledrejection', (event) => recordWindowError('unhandled-rejection', event));
window.__gpuRecoveryTrace = state;
window.__gpuRecoveryDevices = deviceObjects;
return {ok: true};
"""

WAIT_FOR_STATUS = """
const done = arguments[arguments.length - 1];
const expected = arguments[0];
const limit = arguments[1];
let settled = false;
const finish = (value) => {
  if (settled) return;
  settled = true;
  window.clearTimeout(timer);
  done(value);
};
const inspect = () => {
  if (settled) return;
  const status = Number(window.recovery.canvas_status());
  if (status === expected) { finish({ok: true, status}); return; }
  if (status === 255) {
    const element = document.getElementById('recovery-error');
    const diagnostic = element ? (element.textContent || '').trim().slice(0, 512) : '';
    finish({ok: false, status, error: diagnostic || 'Rust recovery reported failure'});
    return;
  }
  window.requestAnimationFrame(inspect);
};
const timer = window.setTimeout(() => finish({
  ok: false,
  status: Number(window.recovery.canvas_status()),
  error: 'status deadline exceeded'
}), limit);
inspect();
"""

DESTROY_AND_WAIT = """
const done = arguments[arguments.length - 1];
const limit = arguments[0];
const devices = window.__gpuRecoveryDevices || [];
const device = devices[0];
if (!device) { done({ok: false, error: 'initial GPU device was not recorded'}); return; }
let settled = false;
const finish = (value) => {
  if (settled) return;
  settled = true;
  window.clearTimeout(timer);
  done(value);
};
const timer = window.setTimeout(
  () => finish({ok: false, error: 'device.lost deadline exceeded'}), limit
);
Promise.resolve(device.lost).then((info) => finish({
  ok: true,
  reason: String(info && info.reason || '').slice(0, 512),
  message: String(info && info.message || '').slice(0, 512)
}), (error) => finish({ok: false, error: String(error).slice(0, 512)}));
device.destroy();
"""

READ_PIXELS = """
const done = arguments[arguments.length - 1];
const id = arguments[0];
const limit = arguments[1];
let settled = false;
const finish = (value) => {
  if (settled) return;
  settled = true;
  window.clearTimeout(timer);
  done(value);
};
const timer = window.setTimeout(
  () => finish({ok: false, error: 'pixel readback deadline exceeded'}), limit
);
let frames = 0;
const capture = () => {
  if (settled) return;
  if (++frames < 2) { window.requestAnimationFrame(capture); return; }
  try {
    const source = document.getElementById(id);
    if (!(source instanceof HTMLCanvasElement)) throw new Error('recovery canvas is missing');
    const copy = document.createElement('canvas');
    copy.width = source.width;
    copy.height = source.height;
    const context = copy.getContext('2d', {willReadFrequently: true});
    if (!context) throw new Error('2D readback context is unavailable');
    context.drawImage(source, 0, 0);
    finish({
      ok: true,
      width: source.width,
      height: source.height,
      pixels: Array.from(context.getImageData(0, 0, source.width, source.height).data)
    });
  } catch (error) {
    finish({ok: false, error: String(error).slice(0, 512)});
  }
};
window.requestAnimationFrame(capture);
"""

READ_SCREENSHOT_PIXELS = """
const done = arguments[arguments.length - 1];
const path = arguments[0];
const limit = arguments[1];
let settled = false;
const finish = (value) => {
  if (settled) return;
  settled = true;
  window.clearTimeout(timer);
  done(value);
};
const timer = window.setTimeout(
  () => finish({ok: false, error: 'screenshot decode deadline exceeded'}), limit
);
const image = new Image();
image.src = new URL(path, document.baseURI).href;
image.decode().then(() => {
  const copy = document.createElement('canvas');
  copy.width = image.naturalWidth;
  copy.height = image.naturalHeight;
  const context = copy.getContext('2d', {willReadFrequently: true});
  if (!context) throw new Error('2D screenshot context is unavailable');
  context.drawImage(image, 0, 0);
  finish({
    ok: true, width: copy.width, height: copy.height,
    pixels: Array.from(context.getImageData(0, 0, copy.width, copy.height).data)
  });
}).catch((error) => finish({ok: false, error: String(error).slice(0, 512)}));
"""
