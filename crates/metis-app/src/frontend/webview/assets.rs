//! Script and markup assets for the packaged `WebView2` pages.

pub(super) const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'self'; style-src 'self'; img-src 'none'; font-src 'none'; media-src 'none'; connect-src 'none'; object-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; manifest-src 'none'; form-action 'none'; base-uri 'none'">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Metis WebView2 form</title>
  <link rel="stylesheet" href="./styles.css">
</head>
<body>
  <main>
    <h1>Metis clinical calculation</h1>
    <p id="host-status" role="status" aria-live="polite">Waiting for the host bridge.</p>
    <form id="calculation" novalidate>
      <label>Patient reference <input id="patient-id" name="patient_id" value="demo" maxlength="128" autocomplete="off" required></label>
      <label>Weight (kg) <input id="weight" name="weight_kg" type="number" min="0" step="any" value="60" required></label>
      <label>Concentration (mg/mL) <input id="concentration" name="concentration_mg_ml" type="number" min="0" step="any" value="2" required></label>
      <label>Target dose (mcg/kg/min) <input id="dose" name="target_dose_mcg_kg_min" type="number" min="0" step="any" value="0.2" required></label>
      <button type="submit">Submit calculation</button>
    </form>
    <p id="result" role="status" aria-live="polite">No calculation submitted.</p>
  </main>
  <script src="./app.js" defer></script>
</body>
</html>
"#;

pub(super) const PERMISSION_PROBE_INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'self'; style-src 'self'; img-src 'none'; font-src 'none'; media-src 'none'; connect-src 'none'; object-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; manifest-src 'none'; form-action 'none'; base-uri 'none'">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Metis WebView2 permission probe</title>
  <link rel="stylesheet" href="./styles.css">
</head>
<body>
  <main>
    <h1>Metis permission probe</h1>
    <p id="host-status" role="status" aria-live="polite">Waiting for the host bridge.</p>
    <p id="result" role="status" aria-live="assertive">Requesting geolocation permission.</p>
  </main>
  <script src="./app.js" defer></script>
</body>
</html>
"#;

pub(super) const STYLES_CSS: &str = r":root { color-scheme: dark; font-family: system-ui, sans-serif; background: #0f172a; color: #e2e8f0; }
body { margin: 0; min-width: 320px; }
main { box-sizing: border-box; width: min(100% - 2rem, 52rem); margin: 0 auto; padding: 2rem 0; }
h1 { color: #67e8f9; }
form { display: grid; gap: 1rem; padding: 1.25rem; border: 1px solid #334155; border-radius: 0.75rem; background: #1e293b; }
label { display: grid; gap: 0.35rem; color: #bae6fd; }
input { box-sizing: border-box; min-height: 2.75rem; border: 1px solid #64748b; border-radius: 0.4rem; background: #0f172a; color: #f8fafc; padding: 0.65rem; font: inherit; }
button { min-height: 2.75rem; border: 0; border-radius: 0.4rem; background: #0891b2; color: #ecfeff; padding: 0.7rem 1rem; font: inherit; font-weight: 700; }
button:disabled { background: #64748b; cursor: not-allowed; }
input:focus-visible, button:focus-visible { outline: 3px solid #facc15; outline-offset: 2px; }
#host-status, #result { min-height: 1.5rem; color: #bae6fd; }
";

pub(super) const APP_JS: &str = r"const form = document.getElementById('calculation');
const status = document.getElementById('host-status');
const result = document.getElementById('result');
const bridge = window.chrome && window.chrome.webview;

function showError(message) {
  result.textContent = message;
  form.querySelector('button').disabled = false;
}

if (!bridge) {
  showError('WebView2 bridge is unavailable.');
  status.textContent = 'Host bridge unavailable';
} else {
  status.textContent = 'Host bridge connected; backend authority remains outside the page.';
  bridge.addEventListener('message', (event) => {
    const message = typeof event.data === 'string' ? JSON.parse(event.data) : event.data;
    if (!message || !['result', 'error'].includes(message.type)) return;
    if (message.type === 'result' && message.status === 'success') {
      result.textContent = `Rate ${message.rate_ml_hr} mL/hour; drug ${message.drug_rate_mg_hr} mg/hour; audit ${message.audit_sequence_id}`;
    } else {
      showError(`${message.status}: ${message.message} [0x${message.error_code.toString(16).padStart(4, '0')}]`);
    }
    form.querySelector('button').disabled = false;
  });
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    form.querySelector('button').disabled = true;
    result.textContent = 'Submitting to the supervised backend…';
    bridge.postMessage({
      action: 'submit',
      patient_id: document.getElementById('patient-id').value,
      weight_kg: Number(document.getElementById('weight').value),
      concentration_mg_ml: Number(document.getElementById('concentration').value),
      target_dose_mcg_kg_min: Number(document.getElementById('dose').value),
    });
  });
}
";

pub(super) const PERMISSION_PROBE_APP_JS: &str = r"const status = document.getElementById('host-status');
const result = document.getElementById('result');
const bridge = window.chrome && window.chrome.webview;

if (!bridge) {
  status.textContent = 'Host bridge unavailable';
  result.textContent = 'Permission probe could not reach the host bridge.';
} else {
  status.textContent = 'Host bridge connected; requesting geolocation for a denial probe.';
  bridge.addEventListener('message', (event) => {
    const message = typeof event.data === 'string' ? JSON.parse(event.data) : event.data;
    if (!message || message.type !== 'error') return;
    result.textContent = `${message.status}: ${message.message} [0x${message.error_code.toString(16).padStart(4, '0')}]`;
  });
  navigator.geolocation.getCurrentPosition(
    () => { result.textContent = 'Permission probe unexpectedly succeeded.'; },
    () => { result.textContent = 'Browser rejected the geolocation request.'; },
  );
}
";
