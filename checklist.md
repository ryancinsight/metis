# Execution

## root
- [Browser lifecycle](backlog.md#METIS-ASYNC-001): Moirai `be87d009cd0e877beef719b47bdcbadc45659069` owns browser callbacks, DOM handles, cancellable local tasks, bounded WebSocket state and deadlines; Metis has the bounded async client/server seam and one-pump out-of-order correlation. Live-service conformance and teardown evidence pass; post-drop allocation, cross-engine, TLS and desktop evidence remain on the board.
- [Browser host](backlog.md#METIS-BROWSER-001): `metis-web` mounts the HTML5/CSS workbench, uses strict external assets, and exposes typed invalid-input/disconnected states. Service-side Origin/session validation and a live backend pass; native desktop, accessibility/IME and cross-engine coverage remain open.
- [Viewer driver](backlog.md#METIS-MIGRATION-001): ritk-snap source audit and DICOM prerequisites recorded; use its real opening/display trace to validate upcoming host, image and asynchronous-loading contracts.
- [Verification](backlog.md#METIS-VERIFY-001): public foundation published; Atlas registration needs a review branch that preserves the shared checkout.
- [Provider](backlog.md#METIS-PROVIDER-001): Moirai browser and WebSocket provider is merged on its default branch at `be87d009cd0e877beef719b47bdcbadc45659069`; consumer lock and standalone verification use the merged revision.
