# metis-app

One application executable composes the backend and presentation libraries.
The parent owns session keys, authorization and audit state; it relaunches its
own executable through Moirai with a presentation role and private pipes.
Both processes contain the same compiled code, but hold separate process state.

```powershell
cargo run --locked -p metis-app -- 60 2 0.2
```

The console demonstration prints a flow of 0.36 mL/hour and a drug rate of
0.72 mg/hour. `--help` prints invocation syntax. The internal
`--metis-frontend` argument selects a child role; it grants no authority.
Windows job containment bounds the managed session, not OS permissions.
On Windows, the visible native host uses the same executable and private IPC.
Its form implements Metis's reusable format-neutral `NativeApplication` host
contract; the platform loop owns the HWND, finite event wait, initial frame and
terminal cleanup while the form keeps text, IME and submit policy:

```powershell
cargo run --locked -p metis-app -- --metis-native-window 60 2 0.2
```

The window paints the real frontend framebuffer, routes Unicode text to the
patient reference while focused, accepts Enter or the authored submit surface,
and handles resize, DPI, focus and close events. The interactive session has a
finite five-minute supervisor budget; it does not grant file, network or device
permissions.

On Windows with the `WebView2` runtime installed, the same executable can host
the packaged HTML5/CSS form. The page-to-host bridge forwards bounded typed
JSON to the unprivileged presentation process, which submits through the same
supervised private IPC and returns the backend result to the page:

```powershell
cargo run --locked -p metis-app -- --metis-webview 60 2 0.2
```

The `WebView` host disables devtools and default context menus, denies new-window
navigation, restricts resources to its generated package directory and grants
no filesystem, network or process authority to page code. Close the window or
press Escape to complete the bounded session.

The same executable can serve one authenticated browser session through the
bounded loopback WebSocket role:

```powershell
cargo run --locked -p metis-app -- --metis-browser-service http://127.0.0.1:8080 8765 66666666666666666666666666666666
```

The browser service accepts the optional `--response-delay-ms MILLISECONDS`
probe flag with a value from 1 through 30,000. It delays successful clinical
responses through Moirai's asynchronous timer so a browser host can verify
stop/remount cancellation against a real service boundary.

The service validates the browser `Origin` before the HTTP upgrade, binds the
configured session context and exits after the peer closes. This role is a
local conformance host; it does not provide TLS or operating-system permission
isolation. The complete browser workflow is in the [user manual](../../docs/manual/browser.md).

The same executable also exposes a bounded loopback HTTP fragment role. It
uses the first-party Moirai HTTP transport and the same origin, session and
capability policy; it serves at most 64 requests before orderly teardown:

```powershell
cargo run --locked -p metis-app -- --metis-http-service http://127.0.0.1:8080 8766 66666666666666666666666666666666
```

The HTTP role accepts the same bounded `--response-delay-ms MILLISECONDS`
probe (1 through 30,000). It delays each response with Moirai's asynchronous
timer so the HTTP page can verify abort, reset and stale-generation handling
against the real service boundary.

`POST /v1/session` accepts a `HandshakeRequestPayload`, and authenticated
`POST /v1/fragments` returns a bounded `FragmentPatchSet` for the registered
UI plugin. `GET /health` is a text-only readiness probe. Every route requires
the exact configured `Origin`; arbitrary HTML, scripts and DICOM data are not
part of this service boundary.

See the [user manual](../../docs/manual/distribution.md) for portable and
installer workflows and the [application decision](../../docs/adr/0006-application-entry.md)
for process boundaries and migration from the removed demonstration commands.
