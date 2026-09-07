# Execution

## root
- [Browser stale-response probe](backlog.md#METIS-BROWSER-002): delayed browser-server response injection remains to be exercised through a real transport and browser lifecycle trace; native window and OS permission providers remain separate.
- [Interactive controls](backlog.md#METIS-INPUT-001): checkbox/radio/range state is Rust-owned through Moirai `ddbd75f`; the live manual trace covers pointer selection, event visibility and keyboard range changes while preserving the backend result. Select/menu/dialog, touch/pointer capture, IME, accessibility and native-host input remain open.
- [Browser lifecycle](backlog.md#METIS-ASYNC-001): Moirai `ddbd75f61914bba195c71cb671bf6d8bf4c14eb6` owns browser callbacks, DOM handles, cancellable local tasks, bounded WebSocket state and deadlines; Metis has the bounded async client/server seam and one-pump out-of-order correlation. Live-service conformance and teardown evidence pass; post-drop allocation, cross-engine, TLS and desktop evidence remain on the board.
- [Browser host](backlog.md#METIS-BROWSER-001): `metis-web` mounts the HTML5/CSS workbench, uses strict external assets, and exposes typed invalid-input/disconnected states. Service-side Origin/session validation and a live backend pass; native desktop, accessibility/IME and cross-engine coverage remain open.
- [Viewer driver](backlog.md#METIS-MIGRATION-001): ritk-snap source audit and DICOM prerequisites recorded; use its real opening/display trace to validate upcoming host, image and asynchronous-loading contracts.
- [Verification](backlog.md#METIS-VERIFY-001): public foundation published; Atlas registration needs a review branch that preserves the shared checkout.
