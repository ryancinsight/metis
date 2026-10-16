//! Script and markup assets for the packaged `WebView2` pages.

pub(super) const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'none'; font-src 'none'; media-src 'none'; connect-src 'none'; object-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; manifest-src 'none'; form-action 'none'; base-uri 'none'">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Metis WebView2 form</title>
  <link rel="stylesheet" href="./styles.css">
</head>
<body data-metis-theme="system">
  <main>
    <h1>Metis clinical calculation</h1>
    <p id="host-status" role="status" aria-live="polite">Waiting for the host bridge.</p>
    <nav id="application-navigation" aria-label="Application navigation">
      <div id="application-toolbar" role="toolbar" aria-label="Application commands">
        <button id="command-menu-toggle" type="button" aria-haspopup="menu" aria-expanded="false" aria-controls="command-menu">Commands</button>
        <button id="command-focus-patient" type="button" aria-keyshortcuts="Alt+Shift+P">Focus patient reference</button>
      </div>
      <div id="command-menu" role="menu" aria-label="Application commands" aria-hidden="true" data-command-menu-open="false">
        <button id="command-theme-dark" role="menuitem" type="button" aria-keyshortcuts="Alt+Shift+D">Use dark theme</button>
        <button id="command-theme-system" role="menuitem" type="button" aria-keyshortcuts="Alt+Shift+S">Use system theme</button>
        <button id="command-copy-result" role="menuitem" type="button" aria-keyshortcuts="Alt+Shift+C">Copy result to clipboard</button>
      </div>
      <p id="command-status" role="status" aria-live="polite">Commands ready</p>
    </nav>
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
  <script type="module" src="./page.js"></script>
</body>
</html>
"#;

pub(super) const PERMISSION_PROBE_INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'none'; font-src 'none'; media-src 'none'; connect-src 'none'; object-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; manifest-src 'none'; form-action 'none'; base-uri 'none'">
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
  <script type="module" src="./page.js"></script>
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
#application-navigation { display: grid; gap: 0.6rem; margin: 1rem 0; }
#application-toolbar { display: flex; flex-wrap: wrap; gap: 0.6rem; }
#command-menu { display: grid; gap: 0.35rem; width: min(100%, 24rem); padding: 0.6rem; border: 1px solid var(--border); border-radius: 0.6rem; background: var(--surface); }
#command-menu[data-command-menu-open="false"] { display: none; }
#command-menu button { width: 100%; text-align: left; }
#command-status { min-height: 1.5rem; margin: 0; color: var(--muted); }
fieldset { display: grid; gap: 0.5rem; margin: 1rem 0; padding: 1rem; border: 1px solid var(--border); border-radius: 0.75rem; background: var(--surface); }
h1 { color: var(--accent-heading); }
form { display: grid; gap: 1rem; padding: 1.25rem; border: 1px solid var(--border); border-radius: 0.75rem; background: var(--surface); }
label { display: grid; gap: 0.35rem; color: var(--muted); }
input, select { box-sizing: border-box; min-height: 2.75rem; border: 1px solid var(--border); border-radius: 0.4rem; background: var(--page); color: var(--control-text); padding: 0.65rem; font: inherit; }
button { min-height: 2.75rem; border: 0; border-radius: 0.4rem; background: var(--accent); color: var(--accent-text); padding: 0.7rem 1rem; font: inherit; font-weight: 700; }
button:disabled { background: var(--surface-raised); color: var(--muted); cursor: not-allowed; }
input:focus-visible, select:focus-visible, button:focus-visible { outline: 3px solid var(--focus); outline-offset: 2px; }
#host-status, #theme-state, #result { min-height: 1.5rem; color: var(--muted); }
@media (max-width: 32rem) {
  #application-toolbar { flex-direction: column; }
  #application-toolbar button { width: 100%; }
}
@media (forced-colors: active) {
  #command-menu { border: 1px solid CanvasText; }
}
"#;

/// The generated WebAssembly page loader: `scripts/webview_page.py` compiles
/// the Rust presentation (the crate `metis-webview-page`) and appends one
/// mechanical auto-init suffix embedding the module bytes for the file://
/// packaged origin.
pub(super) const PAGE_JS: &str = include_str!("generated/page.js");
