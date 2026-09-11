const endpoint = new URL("http://127.0.0.1:8766/health");
const status = document.getElementById("metis-status");
const response = document.getElementById("http-response");
const origin = document.getElementById("http-origin");
const button = document.getElementById("metis-health");

origin.textContent = window.location.origin;

async function probe() {
  button.disabled = true;
  status.textContent = "Probing the local Metis service…";
  response.textContent = "—";
  try {
    const result = await fetch(endpoint, { cache: "no-store" });
    const body = await result.text();
    if (!result.ok || body !== "metis-http-ready\n") {
      throw new Error(`unexpected response ${result.status}`);
    }
    status.textContent = "Loopback HTTP boundary ready";
    response.textContent = `${result.status} ${body.trim()}`;
  } catch (error) {
    status.textContent = "Loopback HTTP boundary unavailable";
    response.textContent = error instanceof Error ? error.message : "request failed";
  } finally {
    button.disabled = false;
  }
}

button.addEventListener("click", probe);
probe();
