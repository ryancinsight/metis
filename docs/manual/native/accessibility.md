> Windows build/run commands require the [current Rust asset preparation](../../../crates/metis-app/src/frontend/webview/manual.md) before compilation. Capture records retain their cited revisions.

## Inspect custom-renderer accessibility semantics

The software renderer validates a host-neutral semantic projection before it
paints a frame. `FrontendApp::semantic_tree` derives bounded roles, names,
descriptions, values, states, focusability and typed actions from the same
markup that produces the pixels:

```rust,no_run
# use metis_frontend::FrontendApp;
# let (transport, _peer) = metis_ipc::MemoryTransport::pair();
# let app = FrontendApp::new(transport, 800, 600)?;
let tree = app.semantic_tree()?;
let patient = tree
    .root
    .children
    .iter()
    .flat_map(|node| node.children.iter())
    .flat_map(|node| node.children.iter())
    .find(|node| node.id.as_deref() == Some("label-patient"))
    .ok_or_else(|| std::io::Error::other("patient input missing"))?;
assert_eq!(patient.role, metis_ui_lang::SemanticRole::TextBox);
assert_eq!(patient.value.as_deref(), Some("PT-9042-ALPHA"));
assert!(patient.focusable);
assert_eq!(patient.actions, [metis_ui_lang::SemanticAction::SetValue]);
let submit = tree
    .root
    .children
    .iter()
    .flat_map(|node| node.children.iter())
    .flat_map(|node| node.children.iter())
    .find(|node| node.id.as_deref() == Some("btn-calc"))
    .ok_or_else(|| std::io::Error::other("submit action missing"))?;
assert!(submit.focusable);
assert_eq!(submit.actions, [metis_ui_lang::SemanticAction::Activate]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Duplicate IDs, unresolved references, malformed state values and oversized
semantic strings fail before presentation. Hidden state applies to the whole
semantic subtree, so a descendant cannot restore focus or actions with
`aria-hidden="false"`; visible siblings retain their normal actions. The
projection is not a native
screen-reader bridge: UIA, NSAccessibility and AT-SPI translation, spoken
output and host preference enablement remain platform-specific acceptance
work under `METIS-A11Y-001`.

### Capture the native semantic projection

The `metis-app` executable can write the production form's host-neutral
semantic tree as a bounded JSON artifact. The command uses the same
`FrontendApp` and software-renderer semantic validation as the visible native
surface, but uses an in-memory transport so it does not start a backend or
claim a platform accessibility bridge:

```powershell
New-Item -ItemType Directory -Force output | Out-Null
$semantic = Join-Path (Resolve-Path .) "output\\native-semantic.json"
cargo run --locked -p metis-app -- --metis-semantic-capture $semantic 60 2 0.2
```

The reviewed specimen is
[`native-semantic.json`](../images/native-semantic.json). It is schema `1`, has
34 elements and 16,246 bytes, and has SHA-256
`75b6c507df5cb1fb9619f837b169e409858eb668236a7b49e85cc35704968eab`.
The `main-screen` application root, the `label-patient` textbox and the
`btn-calc` submit button are present. The textbox exposes its bounded current
value and typed `set_value` action; the button is focusable, enabled and
exposes the typed `activate` action. The artifact is a deterministic
host-neutral projection. It does not establish
UIA, NSAccessibility or AT-SPI translation, spoken screen-reader output,
platform preference enablement or assistive-technology acceptance; those
remain the native residuals in `METIS-A11Y-001`.

### Use the Windows native accessibility bridge

The Windows native role now forwards the validated semantic tree to Moirai's
AccessKit provider. `NativeForm` supplies the tree before the HWND becomes
visible, so the provider never exposes a partially initialized control tree.
Event-loop updates replace the bounded tree after state changes, and closing
and reopening a surface reinstalls the retained tree:

```rust,no_run
use metis_platform::native::{
    AccessibilityAction, AccessibilityNode, AccessibilityRole, AccessibilityTree,
    NativeSurface, WindowConfig, WindowVisibility,
};

let config = WindowConfig::with_visibility("Metis", 800, 600, WindowVisibility::Hidden)?;
let mut button = AccessibilityNode::new(2, AccessibilityRole::Button, "Submit")?;
button.set_focusable(true);
button.add_action(AccessibilityAction::Activate);
let mut root = AccessibilityNode::new(1, AccessibilityRole::Application, "Metis")?;
root.set_children(vec![2])?;
let tree: AccessibilityTree = AccessibilityTree::from_nodes(1, 2, vec![root, button])?;
let mut surface = NativeSurface::new_with_accessibility(&config, Some(tree.clone()))?;
surface.update_accessibility(tree)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The authored form maps its stable `btn-calc` identity to `Activate` and its
stable `label-patient` identity to `Focus` and bounded `SetValue` handling.
Both routes use the same bounded native event queue as keyboard input; an
accepted patient value repaints the frontend and updates the semantic value.
Oversized or control-bearing replacements are rejected before state mutation.
Navigation, complementary, toolbar, menu and menu-item roles map through the
same format-neutral contract; Moirai PR [#432](https://github.com/ryancinsight/Moirai/pull/432)
(merge `0e2e1bbb2d81e16dd9c694ba46a9e9710e034417`) maps them to AccessKit.
Projection rejects duplicate or zero identities, oversized strings and future
semantic roles/actions that are not in the admitted native vocabulary.
The provider implementation is in Moirai (PRs [#410](https://github.com/ryancinsight/Moirai/pull/410),
[#411](https://github.com/ryancinsight/Moirai/pull/411) and [#414](https://github.com/ryancinsight/Moirai/pull/414)); Metis keeps the
semantic source and application action policy. The component evidence covers
tree installation, updates and reopen; it does not claim that a specific
installed screen reader speaks or operates the form.

### Exercise the Windows UI Automation journey

The optional Windows runner exercises the visible production executable through
the operating system's UI Automation client. It discovers the real `Patient ID`
edit and submit button, sends a bounded `ValuePattern.SetValue` request, invokes
the button with `InvokePattern.Invoke`, waits for the backend result and captures
the window before and after the actions. The runner exits nonzero when the
required controls, value transition or result are absent, and limits the
Automation tree, value bytes and PowerShell response. Run it from the Metis
repository after building `metis-app`:

```powershell
cargo build --locked -p metis-app --bin metis-app
$target = (cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json).target_directory
$binary = Join-Path $target "debug\metis-app.exe"
New-Item -ItemType Directory -Force output/native-uia | Out-Null
python -S scripts/python_native_accessibility.py `
  --command $binary `
  --argument=--metis-native-window --argument=60 --argument=2 --argument=0.2 `
  --initial-output output/native-uia/initial.png `
  --output output/native-uia/after.png `
  --manifest output/native-uia/trace.json
```

The output directory is ignored run output. A local Windows run on 2026-09-21
used an 818×647 window at 120 DPI and found 10 named UI Automation nodes. It
observed `demo` → `UIA-PATIENT`, then the backend result
`Backend Calculation Output Rate: 0.360 mL/hr (0.72 mg/hr)`; the initial and
after PNGs changed pixels and hash to
`60adf0767bb162e55ea9f21351b18a16d1c20aa870cad9c631abd3eb34109962` and
`54ccf53bda8713b130a50cf916d32c11b137c336fb271c80c1c3b9e6a47df1af`.
The locked build resolved all Moirai packages at merged PR [#414](https://github.com/ryancinsight/Moirai/pull/414),
revision `d7b38d7`, and its `Cargo.lock` SHA-256 is
`95bf4b497677c141308de3ad4d44e3be58e90a000d4eb48befb524779009c568`.
The focused locked native gate passed formatting, warning-denied Clippy and
32/32 `metis-app` nextest cases before this capture. The trace proves the
native provider action path and visible result; it does not prove installed
screen-reader speech, host preference enablement or non-Windows accessibility.

### Exercise the packaged WebView2 controls

The same runner accepts `--surface webview2` for the real packaged form. It
focuses the host HWND, sends two bounded `Tab` actions to activate WebView2's
renderer accessibility provider, then finds the page's `Patient reference`
edit, `Submit calculation` button and `result` status bar. `ValuePattern.SetValue`
changes `demo` to `A11Y-V03`; `InvokePattern.Invoke` submits the form; the
status bar exposes `Rate 0.36 mL/hour; drug 0.72 mg/hour; audit 2`; and the
window closes with process return code `0`.

```powershell
$target = (cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json).target_directory
$binary = Join-Path $target "debug\metis-app.exe"
New-Item -ItemType Directory -Force output/native-uia | Out-Null
python -S scripts/python_native_accessibility.py `
  --surface webview2 `
  --command $binary `
  --argument=--metis-webview --argument=60 --argument=2 --argument=0.2 `
  --patient-value A11Y-V03 `
  --initial-output output/native-uia/webview-initial.png `
  --output output/native-uia/webview-after.png `
  --manifest output/native-uia/webview-trace.json
```

The bounded record is [`webview-uia.json`](../images/webview-uia.json). It binds
the application revision, executable and lock digests, WebView2 runtime,
window geometry, 29 named UI Automation nodes, the value transition, terminal
status and orderly close. Rendering is measured with WebView2 `CapturePreview`
(1025×769, 12,883 bytes, SHA-256
`1b9c3bfde1c32c374edfcfab0303df3742c3772881d1c06aaf345b62e295282e`); the
parent-HWND GDI capture used by the native-only runner is not treated as a
WebView2 rendering oracle. This is control and rendering evidence only: it
does not assert spoken output or screen-reader certification, and no
screen-reader process is started by this procedure.


[Windows native manual](../native.md) · [Current Rust page contract](../../../crates/metis-app/src/frontend/webview/manual.md)
