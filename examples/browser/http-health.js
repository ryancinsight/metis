const healthEndpoint = new URL("http://127.0.0.1:8766/health");
const sessionEndpoint = new URL("http://127.0.0.1:8766/v1/session");
const fragmentEndpoint = new URL("http://127.0.0.1:8766/v1/fragments");
const protocolVersion = 0x0100;
const principal = new Uint8Array(16).fill(0x66);
const tokenBytes = 84;
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder("utf-8", { fatal: true });
const allowedTargets = new Set(["metis-events", "metis-status"]);

const status = document.getElementById("metis-status");
const response = document.getElementById("http-response");
const origin = document.getElementById("http-origin");
const healthButton = document.getElementById("metis-health");
const fragmentButton = document.getElementById("metis-fragment");
const resetButton = document.getElementById("metis-reset");
const fragmentInput = document.getElementById("fragment-input");
const events = document.getElementById("metis-events");
const negative = document.getElementById("metis-negative");
const lifecycle = document.getElementById("metis-lifecycle");

let sessionToken;
let mountGeneration = 1;
let requestController;

origin.textContent = window.location.origin;

function requireLength(view, offset, length) {
  if (length < 0 || offset > view.byteLength - length) {
    throw new Error("Metis payload is truncated");
  }
}

function encodeHandshake() {
  const buffer = new ArrayBuffer(22);
  const view = new DataView(buffer);
  view.setUint16(0, protocolVersion);
  view.setUint32(2, 42);
  new Uint8Array(buffer, 6).set(principal);
  return buffer;
}

function decodeHandshake(buffer) {
  if (buffer.byteLength !== 2 + tokenBytes) {
    throw new Error("Metis handshake length is invalid");
  }
  const view = new DataView(buffer);
  if (view.getUint16(0) !== protocolVersion) {
    throw new Error("Metis handshake version is invalid");
  }
  const tokenPrincipal = new Uint8Array(buffer, 10, principal.byteLength);
  if (!tokenPrincipal.every((value, index) => value === principal[index])) {
    throw new Error("Metis handshake principal is invalid");
  }
  if ((view.getUint32(26) & 1) === 0) {
    throw new Error("Metis handshake lacks UI render scope");
  }
  return new Uint8Array(buffer.slice(2));
}

function encodeAction(generation, input) {
  const action = textEncoder.encode("status.describe");
  const target = textEncoder.encode("metis-events");
  const value = textEncoder.encode(input);
  if (generation === 0 || value.byteLength > 4096) {
    throw new Error("Fragment input exceeds its bound");
  }
  const buffer = new ArrayBuffer(16 + action.byteLength + target.byteLength + value.byteLength);
  const view = new DataView(buffer);
  view.setBigUint64(0, BigInt(generation));
  view.setUint16(8, action.byteLength);
  view.setUint16(10, target.byteLength);
  view.setUint32(12, value.byteLength);
  const body = new Uint8Array(buffer, 16);
  body.set(action, 0);
  body.set(target, action.byteLength);
  body.set(value, action.byteLength + target.byteLength);
  return buffer;
}

function encodeInvocation(token, action) {
  const plugin = textEncoder.encode("ui");
  const operation = textEncoder.encode("action");
  const buffer = new ArrayBuffer(tokenBytes + 8 + plugin.byteLength + operation.byteLength + action.byteLength);
  const view = new DataView(buffer);
  new Uint8Array(buffer, 0, tokenBytes).set(token);
  view.setUint16(tokenBytes, plugin.byteLength);
  view.setUint16(tokenBytes + 2, operation.byteLength);
  view.setUint32(tokenBytes + 4, action.byteLength);
  const body = new Uint8Array(buffer, tokenBytes + 8);
  body.set(plugin, 0);
  body.set(operation, plugin.byteLength);
  body.set(new Uint8Array(action), plugin.byteLength + operation.byteLength);
  return buffer;
}

async function requestBinary(url, body, signal) {
  const result = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/metis" },
    body,
    cache: "no-store",
    signal,
  });
  return { status: result.status, body: await result.arrayBuffer() };
}

function readError(buffer) {
  if (buffer.byteLength < 4) {
    return "Metis error payload is truncated";
  }
  const view = new DataView(buffer);
  const length = view.getUint16(2);
  if (4 + length !== buffer.byteLength) {
    return "Metis error payload length is invalid";
  }
  try {
    return `error ${view.getUint16(0)} ${textDecoder.decode(new Uint8Array(buffer, 4))}`;
  } catch {
    return "Metis error payload is not UTF-8";
  }
}

function readString(view, cursor, width, maximum, label) {
  requireLength(view, cursor.value, width);
  const length = width === 2 ? view.getUint16(cursor.value) : view.getUint32(cursor.value);
  cursor.value += width;
  if (length > maximum) {
    throw new Error(`${label} exceeds its bound`);
  }
  requireLength(view, cursor.value, length);
  const value = textDecoder.decode(new Uint8Array(view.buffer, view.byteOffset + cursor.value, length));
  cursor.value += length;
  return value;
}

function decodePatchSet(buffer) {
  const view = new DataView(buffer);
  const cursor = { value: 0 };
  requireLength(view, cursor.value, 12);
  const generationValue = view.getBigUint64(cursor.value);
  if (generationValue > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error("Metis patch generation cannot be represented safely");
  }
  const generation = Number(generationValue);
  cursor.value += 8;
  const count = view.getUint16(cursor.value);
  cursor.value += 2;
  if (view.getUint16(cursor.value) !== 0) {
    throw new Error("Metis patch reserved bytes are non-zero");
  }
  cursor.value += 2;
  if (generation === 0 || count > 32) {
    throw new Error("Metis patch set exceeds its bound");
  }
  const patches = [];
  for (let index = 0; index < count; index += 1) {
    requireLength(view, cursor.value, 1);
    const kind = view.getUint8(cursor.value);
    cursor.value += 1;
    if (kind === 1 || kind === 3) {
      const target = readString(view, cursor, 2, 64, "Fragment target");
      const value = readString(view, cursor, 4, 4096, "Fragment value");
      patches.push({ kind, target, value });
    } else if (kind === 2) {
      const target = readString(view, cursor, 2, 64, "Fragment target");
      const name = readString(view, cursor, 2, 64, "Fragment attribute");
      const value = readString(view, cursor, 4, 4096, "Fragment value");
      patches.push({ kind, target, name, value });
    } else {
      throw new Error("Metis patch kind is unknown");
    }
  }
  if (cursor.value !== view.byteLength) {
    throw new Error("Metis patch payload has trailing bytes");
  }
  return { generation, patches };
}

function attributeAllowed(name) {
  return name === "class" || name === "value" || name.startsWith("aria-") || name.startsWith("data-");
}

function applyPatchSet(patchSet) {
  if (patchSet.generation !== mountGeneration) {
    throw new Error("stale fragment generation");
  }
  const updates = patchSet.patches.map((patch) => {
    if (!allowedTargets.has(patch.target)) {
      throw new Error("fragment target is not allowlisted");
    }
    const element = document.getElementById(patch.target);
    if (!element) {
      throw new Error("fragment target is not mounted");
    }
    if (patch.kind === 2 && !attributeAllowed(patch.name)) {
      throw new Error("fragment attribute is not allowlisted");
    }
    return { element, patch };
  });
  for (const { element, patch } of updates) {
    if (patch.kind === 2) {
      element.setAttribute(patch.name, patch.value);
    } else {
      element.textContent = patch.value;
    }
  }
}

async function openSession() {
  if (sessionToken) {
    return sessionToken;
  }
  const result = await requestBinary(sessionEndpoint, encodeHandshake());
  if (result.status !== 200) {
    throw new Error(`handshake ${result.status}: ${readError(result.body)}`);
  }
  sessionToken = decodeHandshake(result.body);
  return sessionToken;
}

async function dispatchFragment(input, generation = mountGeneration, signal) {
  const token = await openSession();
  const action = encodeAction(generation, input);
  const result = await requestBinary(fragmentEndpoint, encodeInvocation(token, action), signal);
  if (result.status !== 200) {
    throw new Error(`fragment ${result.status}: ${readError(result.body)}`);
  }
  return decodePatchSet(result.body);
}

async function runNegativeProbes() {
  const malformed = await requestBinary(fragmentEndpoint, new Uint8Array());
  if (malformed.status !== 400) {
    throw new Error(`malformed probe returned ${malformed.status}`);
  }
  const unauthorized = await requestBinary(
    fragmentEndpoint,
    encodeInvocation(new Uint8Array(tokenBytes), encodeAction(mountGeneration, fragmentInput.value)),
  );
  if (unauthorized.status !== 401) {
    throw new Error(`unauthorized probe returned ${unauthorized.status}`);
  }
  const before = events.textContent;
  const stale = await dispatchFragment(fragmentInput.value, mountGeneration + 1);
  let staleRejected = false;
  try {
    applyPatchSet(stale);
  } catch (error) {
    staleRejected = error instanceof Error && error.message === "stale fragment generation";
  }
  if (!staleRejected || events.textContent !== before) {
    throw new Error("stale fragment was applied");
  }
  negative.textContent = "malformed 400 · unauthorized 401 · stale unchanged";
}

async function runFragment() {
  requestController?.abort();
  requestController = new AbortController();
  fragmentButton.disabled = true;
  try {
    const patchSet = await dispatchFragment(fragmentInput.value, mountGeneration, requestController.signal);
    applyPatchSet(patchSet);
    response.textContent = `200 metis-http-ready · handshake 200 · fragment 200 (${patchSet.patches.length} patch)`;
    status.textContent = "Authenticated fragment boundary ready";
  } catch (error) {
    status.textContent = "Authenticated fragment boundary unavailable";
    response.textContent = error instanceof Error ? error.message : "fragment request failed";
  } finally {
    fragmentButton.disabled = !sessionToken;
  }
}

async function probe() {
  healthButton.disabled = true;
  fragmentButton.disabled = true;
  status.textContent = "Probing the local Metis service…";
  response.textContent = "—";
  negative.textContent = "—";
  try {
    const result = await fetch(healthEndpoint, { cache: "no-store" });
    const body = await result.text();
    if (!result.ok || body !== "metis-http-ready\n") {
      throw new Error(`health ${result.status}`);
    }
    await openSession();
    await runFragment();
    await runNegativeProbes();
  } catch (error) {
    status.textContent = "Loopback HTTP boundary unavailable";
    response.textContent = error instanceof Error ? error.message : "request failed";
  } finally {
    healthButton.disabled = false;
    fragmentButton.disabled = !sessionToken;
  }
}

function resetMount() {
  requestController?.abort();
  if (mountGeneration === Number.MAX_SAFE_INTEGER) {
    status.textContent = "Mount generation exhausted";
    return;
  }
  mountGeneration += 1;
  lifecycle.textContent = `generation ${mountGeneration}`;
  status.textContent = "Mount reset; the previous fragment generation is stale";
}

healthButton.addEventListener("click", probe);
fragmentButton.addEventListener("click", runFragment);
resetButton.addEventListener("click", resetMount);
probe();
