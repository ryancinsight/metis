import init from './canvas_input.js';
try {
  await init();
} catch (error) {
  const status = document.getElementById('canvas-input-status');
  if (status) status.textContent = `load-error:${String(error).slice(0, 512)}`;
}
