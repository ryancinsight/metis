# Metis

Metis is an unfinished Rust desktop presentation and isolated backend workspace
inside Atlas. Its name refers to Metis, associated with counsel and practical
wisdom. It uses Rust and a bounded HTML/CSS-inspired language; it does not embed
JavaScript or a WebView.

The current implementation provides binary IPC, session capabilities, backend
calculation and audit ownership, a software rasterizer and a headless form
workflow. It does not yet provide Tauri feature parity, native desktop windows,
an OS privilege sandbox, durable audit storage or regulatory certification.
The infusion arithmetic is a synthetic engineering example; its configurable
limits are not treatment guidance.

## Build and run

Use the pinned Rust toolchain and cargo-nextest 0.9.143. From this directory:

```text
cargo build --workspace --bins
cargo run -p metis-backend -- 60 2 0.2
python scripts/verify.py --stack
```

The arguments are weight in kg, concentration in mg/mL and dose in mcg/kg/min.
The stack gate uses local Atlas providers; standalone verification omits `--stack`.
The backend launches a separate frontend executable, receives its submitted
values and returns the calculated result. For these demonstration inputs the
rate is 0.36 mL/hour and drug rate is 0.72 mg/hour. Standard output carries IPC
bytes exclusively; human-readable results go to standard error.

## Atlas ownership

Moirai owns worker scheduling and process lifecycle. Metis consumes its executor
and transport APIs; missing process-pipe/deadline capabilities are implemented
upstream in Moirai. Iris supplies the `RenderBackend` contract; Metis implements
its bounded software renderer against that contract. Frontend dependencies never
include `metis-backend`.

Metis declares no direct third-party crates. Atlas providers have transitive
dependencies; the gate records the actual graph instead of describing it as
dependency-free. The Atlas development overlay resolves first-party code to local
trees. Standalone builds depend on the corresponding pushed provider revisions.
Moirai is pinned to the pushed process-support revision until its 0.6 API reaches
the default branch; the removal trigger is tracked in
[the board](backlog.md#METIS-PROVIDER-001).

## Design and evidence

- [Architecture](docs/ARCHITECTURE.md)
- [Interface contract](docs/INTERFACE.md)
- [Risk controls](docs/RISK_CONTROLS.md)
- [Verification](docs/VERIFICATION.md)
- [Current work](backlog.md)
