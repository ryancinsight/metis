> Windows build/run commands require the [current Rust asset preparation](../../../crates/metis-app/src/frontend/webview/manual.md) before compilation. Capture records retain their cited revisions.

# Run the Windows native surface

The Windows platform crate now has a safe adapter over Moirai's thread-owned
Win32 provider. It owns one native window, translates its messages into bounded
`WindowEvent` values and presents the same row-major `0xAARRGGBB` pixels used by
`Framebuffer`. The adapter does not host HTML/CSS, grant file or network access,
or change the portable `PlatformEvent` queue.

## Run the visible form

The application entry composes the adapter with the existing frontend and the
supervised backend process. On Windows, run:

```powershell
cargo run --locked -p metis-app -- --metis-native-window 60 2 0.2
```

The `Metis native form` window renders the production software framebuffer.
The form opens with focus on the patient reference: typed Unicode characters
extend it and Backspace removes its last scalar; every edit clears a prior
calculation through `FrontendApp::set_inputs`. **Tab** and **Shift+Tab** move
focus through the controls and paint a focus ring; typing reaches the patient
reference only while it holds focus. **Enter** or **Space** activates a focused
button, and on the Commands button opens the menu with focus on its first
item. With the patient reference focused, press **Enter** or **Ctrl+Enter**,
or click the blue **Submit calculation** surface, to send the
exact numeric inputs through the private pipe. Clicking a control focuses it
without a ring. Alt/Windows-modified Enter is
left to the operating system instead of triggering the application shortcut.
The result and audit sequence are painted by the same frontend state machine as
the headless workflow. Resize the window to exercise framebuffer replacement;
DPI, focus and close events are consumed by the host. **Escape** or the window
close control ends the child cleanly. When the authored **Commands** menu is
open, it floats at the laid-out Commands button without moving the form.
Clicking outside it or switching away from the window dismisses it; an outside
click does not activate the underlying control. Escape closes that menu first;
a second Escape closes the child. The
toolbar's **Focus patient** command moves native input focus to the patient
reference. Its menu also applies the bounded dark or system presentation theme;
these actions mutate `FrontendApp` state and never cross the backend IPC seam.

A host that owns the validated `WindowConfig` can call
`NativeSurface::reopen` after `close` to create a fresh HWND with the same
bounded configuration; reopening a live surface is rejected.

The parent keeps this interactive session under a finite five-minute watchdog
budget so an abandoned window cannot leave a process tree running forever.
This role is Windows-only. Native IME start/update/commit/cancel phases are
consumed by the host; preedit text stays transient and committed UTF-8 text uses
the same bounded patient-field transition as ordinary text input. WebView2
composition remains a separate host role.

The application boundary regression sends the same bounded provider events
through `NativeForm`: start/update changes only the transient preedit, focus
loss cancels it without changing the patient field, and commit applies the
UTF-8 text through the ordinary bounded transition. This is consumer contract
evidence; it does not claim that an installed CJK or other system IME was
exercised. The remaining physical journey is described in the [real Unicode
input capture](input.md#capture-real-unicode-input).


[Windows native manual](../native.md) · [Current Rust page contract](../../../crates/metis-app/src/frontend/webview/manual.md)
