import init from './canvas_input.js';
try {
  window.canvasInput = await init();
  const canvas = document.getElementById('canvas-input');
  const add = EventTarget.prototype.addEventListener, remove = EventTarget.prototype.removeEventListener;
  const entries = [], observations = [];
  const capture = (options) => typeof options === 'boolean' ? options : Boolean(options && options.capture);
  EventTarget.prototype.addEventListener = function(type, listener, options) {
    if (this !== canvas || typeof listener !== 'function') {
      return add.call(this, type, listener, options);
    }
    const entry = {type, listener, capture: capture(options), active: true, wrapped: null};
    entry.wrapped = function(event) {
      const observation = {
        type, pointer_id: event.pointerId ?? null, button: event.button ?? null, buttons: event.buttons ?? null,
        trusted: event.isTrusted === true, returned: false, default_prevented: null, has_capture: null,
      };
      observations.push(observation);
      listener.call(this, event);
      Object.assign(observation, {returned: true, default_prevented: event.defaultPrevented === true,
        has_capture: Number.isInteger(event.pointerId) ? canvas.hasPointerCapture(event.pointerId) : null});
    };
    entries.push(entry);
    return add.call(this, type, entry.wrapped, options);
  };
  EventTarget.prototype.removeEventListener = function(type, listener, options) {
    if (this !== canvas) return remove.call(this, type, listener, options);
    const registered = entries.find((entry) => entry.active && entry.type === type &&
      entry.listener === listener && entry.capture === capture(options));
    if (!registered) return remove.call(this, type, listener, options);
    const result = remove.call(this, type, registered.wrapped, options);
    registered.active = false;
    return result;
  };
  window.canvasInputTrace = {
    state: () => ({registrations: entries.length, active: entries.filter((entry) => entry.active).length,
      removals: entries.filter((entry) => !entry.active).length, observations: observations.slice()}),
    restore: () => { EventTarget.prototype.addEventListener = add;
      EventTarget.prototype.removeEventListener = remove; },
  };
  for (const action of ['start', 'drain', 'stop']) document.getElementById(`canvas-${action}`).addEventListener('click', () => window.canvasInput[`canvas_${action}`]());
  document.getElementById('canvas-input-status').textContent = 'loaded';
} catch (error) {
  const status = document.getElementById('canvas-input-status');
  if (status) status.textContent = `load-error:${String(error).slice(0, 512)}`;
}
