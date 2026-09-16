// RITK owns the viewer loop and consumes Metis's bounded byte handoff.
const status = document.getElementById("gallery-status");
try {
  const { default: init, start_web_orthogonal_canvases, stop_web_canvas,
    web_canvas_listener_count } =
    await import("./consumer/ritk_snap.js");
  const runtime = await init();
  let mounted = false;
  const stop = () => {
    stop_web_canvas();
    mounted = false;
    status.textContent = "Stopped. Viewer resources released.";
  };
  const mount = () => {
    stop();
    start_web_orthogonal_canvases(
      "ritk-snap-axial", "ritk-snap-coronal", "ritk-snap-sagittal",
    );
    mounted = true;
    status.textContent = "Ready. Drop study files into the area below.";
  };
  window.metisGallery = Object.freeze({
    mount, stop,
    sample: () => ({
      mounted,
      wasm_bytes: runtime.memory.buffer.byteLength,
      host_listeners: Number(document.getElementById("metis-app")
        .getAttribute("data-metis-listener-count")),
      consumer_listeners: web_canvas_listener_count(),
    }),
  });
  mount();
  window.addEventListener("pagehide", stop, { once: true });
} catch (error) {
  status.textContent = `Viewer could not start: ${error.message}`;
  status.setAttribute("data-state", "failed");
}
