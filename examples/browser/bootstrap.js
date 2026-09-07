import init from "./metis_web.js";

const wasm = await init();
wasm.metis_start();

document.getElementById("metis-start").addEventListener("click", () => wasm.metis_start());
document.getElementById("metis-stop").addEventListener("click", () => wasm.metis_stop());

document.addEventListener("click", (event) => {
  if (!(event.target instanceof Element)) {
    return;
  }
  const link = event.target.closest("a[href]");
  if (!link) {
    return;
  }
  const destination = new URL(link.href, window.location.href);
  if (destination.origin !== window.location.origin) {
    event.preventDefault();
  }
});
