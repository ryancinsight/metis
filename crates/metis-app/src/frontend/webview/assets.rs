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
<body data-metis-theme="system">
  <main>
    <h1>Metis clinical calculation</h1>
    <p id="host-status" role="status" aria-live="polite">Waiting for the host bridge.</p>
    <fieldset id="view-options">
      <legend>View options</legend>
      <label for="theme-mode">Theme</label>
      <select id="theme-mode" name="theme-mode">
        <option value="system" selected>System preference</option>
        <option value="light">Light</option>
        <option value="dark">Dark</option>
        <option value="high-contrast">High contrast</option>
      </select>
      <p id="theme-state" role="status" aria-live="polite">Theme: system preference</p>
    </fieldset>
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
<body data-metis-theme="system">
  <main>
    <h1>Metis permission probe</h1>
    <p id="host-status" role="status" aria-live="polite">Waiting for the host bridge.</p>
    <p id="result" role="status" aria-live="assertive">Preparing bounded capability requests.</p>
    <ol id="permission-results" aria-label="Capability denial results">
      <li id="permission-geolocation">Geolocation: pending</li>
      <li id="permission-camera">Camera: pending</li>
      <li id="permission-microphone">Microphone: pending</li>
      <li id="permission-notifications">Notifications: pending</li>
    </ol>
  </main>
  <script src="./app.js" defer></script>
</body>
</html>
"#;

pub(super) const STYLES_CSS: &str = r#":root { font-family: system-ui, sans-serif; }
body { --page: #f8fafc; --surface: #ffffff; --surface-raised: #e2e8f0; --text: #0f172a; --control-text: #0f172a; --muted: #334155; --accent: #0369a1; --accent-heading: #075985; --accent-text: #ffffff; --border: #64748b; --focus: #b45309; color-scheme: light; margin: 0; min-width: 320px; background: var(--page); color: var(--text); }
body[data-metis-theme="light"] { color-scheme: light; }
body[data-metis-theme="dark"] { --page: #0f172a; --surface: #1e293b; --surface-raised: #334155; --text: #e2e8f0; --control-text: #f8fafc; --muted: #bae6fd; --accent: #0891b2; --accent-heading: #67e8f9; --accent-text: #ecfeff; --border: #64748b; --focus: #facc15; color-scheme: dark; }
body[data-metis-theme="high-contrast"] { --page: #000000; --surface: #000000; --surface-raised: #000000; --text: #ffffff; --control-text: #ffffff; --muted: #ffffff; --accent: #ffff00; --accent-heading: #ffff00; --accent-text: #000000; --border: #ffffff; --focus: #00ffff; color-scheme: only dark; }
@media (prefers-color-scheme: dark) {
  body[data-metis-theme="system"] { --page: #0f172a; --surface: #1e293b; --surface-raised: #334155; --text: #e2e8f0; --control-text: #f8fafc; --muted: #bae6fd; --accent: #0891b2; --accent-heading: #67e8f9; --accent-text: #ecfeff; --border: #64748b; --focus: #facc15; color-scheme: dark; }
}
main { box-sizing: border-box; width: min(100% - 2rem, 52rem); margin: 0 auto; padding: 2rem 0; }
fieldset { display: grid; gap: 0.5rem; margin: 1rem 0; padding: 1rem; border: 1px solid var(--border); border-radius: 0.75rem; background: var(--surface); }
h1 { color: var(--accent-heading); }
form { display: grid; gap: 1rem; padding: 1.25rem; border: 1px solid var(--border); border-radius: 0.75rem; background: var(--surface); }
label { display: grid; gap: 0.35rem; color: var(--muted); }
input, select { box-sizing: border-box; min-height: 2.75rem; border: 1px solid var(--border); border-radius: 0.4rem; background: var(--page); color: var(--control-text); padding: 0.65rem; font: inherit; }
button { min-height: 2.75rem; border: 0; border-radius: 0.4rem; background: var(--accent); color: var(--accent-text); padding: 0.7rem 1rem; font: inherit; font-weight: 700; }
button:disabled { background: var(--surface-raised); color: var(--muted); cursor: not-allowed; }
input:focus-visible, select:focus-visible, button:focus-visible { outline: 3px solid var(--focus); outline-offset: 2px; }
#host-status, #theme-state, #result { min-height: 1.5rem; color: var(--muted); }
"#;

pub(super) const APP_JS: &str = r"const form = document.getElementById('calculation');
const themeMode = document.getElementById('theme-mode');
const themeState = document.getElementById('theme-state');
const status = document.getElementById('host-status');
const result = document.getElementById('result');
const bridge = window.chrome && window.chrome.webview;
const themes = new Map([
  ['system', 'system preference'],
  ['light', 'light'],
  ['dark', 'dark'],
  ['high-contrast', 'high contrast'],
]);

function applyTheme(value) {
  const label = themes.get(value);
  if (!label) return;
  document.body.dataset.metisTheme = value;
  themeState.textContent = `Theme: ${label}`;
}

const requestedTheme = new URLSearchParams(window.location.search).get('theme')
  || document.body.dataset.metisTheme;
if (themes.has(requestedTheme)) themeMode.value = requestedTheme;
applyTheme(themeMode.value);
themeMode.addEventListener('change', () => applyTheme(themeMode.value));

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
const rows = new Map([
  ['geolocation', document.getElementById('permission-geolocation')],
  ['camera', document.getElementById('permission-camera')],
  ['microphone', document.getElementById('permission-microphone')],
  ['notifications', document.getElementById('permission-notifications')],
]);
const probes = [
  {
    permission: 'geolocation',
    invoke: () => navigator.geolocation
      ? new Promise((resolve) => navigator.geolocation.getCurrentPosition(
          () => resolve('unexpected success'),
          () => resolve('browser rejected the request'),
        ))
      : Promise.resolve('API unavailable'),
  },
  {
    permission: 'camera',
    invoke: () => navigator.mediaDevices
      ? navigator.mediaDevices.getUserMedia({ video: true }).then((stream) => {
          for (const track of stream.getTracks()) track.stop();
          return 'unexpected success';
        }, () => 'browser rejected the request')
      : Promise.resolve('API unavailable'),
  },
  {
    permission: 'microphone',
    invoke: () => navigator.mediaDevices
      ? navigator.mediaDevices.getUserMedia({ audio: true }).then((stream) => {
          for (const track of stream.getTracks()) track.stop();
          return 'unexpected success';
        }, () => 'browser rejected the request')
      : Promise.resolve('API unavailable'),
  },
  {
    permission: 'notifications',
    invoke: () => window.Notification
      ? Notification.requestPermission().then((permission) => `browser result: ${permission}`)
      : Promise.resolve('API unavailable'),
  },
];
let active = 0;
const recorded = new Set();

function record(permission, text) {
  const row = rows.get(permission);
  if (row) row.textContent = `${permission}: ${text}`;
  recorded.add(permission);
}

function runNext() {
  if (active >= probes.length) {
    result.textContent = `Completed ${recorded.size} bounded capability probes.`;
    bridge.postMessage({ action: 'permission_probe_complete' });
    return;
  }
  const probe = probes[active++];
  result.textContent = `Requesting ${probe.permission} permission.`;
  Promise.resolve().then(() => probe.invoke()).then((text) => {
    if (!recorded.has(probe.permission)) record(probe.permission, text);
    runNext();
  }, () => {
    if (!recorded.has(probe.permission)) record(probe.permission, 'browser rejected the request');
    runNext();
  });
}

if (!bridge) {
  status.textContent = 'Host bridge unavailable';
  result.textContent = 'Permission probe could not reach the host bridge.';
} else {
  status.textContent = 'Host bridge connected; every capability request is denied by policy.';
  bridge.addEventListener('message', (event) => {
    const message = typeof event.data === 'string' ? JSON.parse(event.data) : event.data;
    if (!message) return;
    if (message.type === 'permission_denied' && rows.has(message.permission)) {
      record(message.permission, `host denied${message.user_initiated ? ' after user action' : ''}`);
      result.textContent = `${message.status}: ${message.message} [0x${message.error_code.toString(16).padStart(4, '0')}]`;
    }
    if (message.type === 'error') {
      result.textContent = `${message.status}: ${message.message} [0x${message.error_code.toString(16).padStart(4, '0')}]`;
    }
  });
  runNext();
}
";
