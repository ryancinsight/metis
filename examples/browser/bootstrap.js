import init from "./metis_web.js";

const parameters = new URLSearchParams(window.location.search);
const configuration = [
  ["metis-websocket-endpoint", parameters.get("endpoint")],
  ["metis-process-id", parameters.get("process")],
  ["metis-principal", parameters.get("principal")],
];
for (const [id, value] of configuration) {
  if (value !== null) {
    document.getElementById(id).value = value;
  }
}

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
