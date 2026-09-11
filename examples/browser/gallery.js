// RITK owns the viewer loop and consumes Metis's bounded byte handoff.
const status = document.getElementById("gallery-status");
try {
  const { default: init, start_web_orthogonal_canvases, stop_web_canvas } =
    await import("./consumer/ritk_snap.js");
  await init();
  start_web_orthogonal_canvases(
    "ritk-snap-axial", "ritk-snap-coronal", "ritk-snap-sagittal",
  );
  window.addEventListener("pagehide", () => stop_web_canvas(), { once: true });
  status.textContent = "Ready. Drop study files into the area below.";
} catch (error) {
  status.textContent = `Viewer could not start: ${error.message}`;
  status.setAttribute("data-state", "failed");
}
