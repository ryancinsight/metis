# Architecture

The shared `metis-core` crate owns wire types, error codes and capability claim
encoding. `metis-ipc` owns framing, canonical payload interpretation, transport
correlation and typed failure reporting. `metis-backend` alone owns calculation
policy, session authorization, audit storage and backend keys.

`metis-frontend` converts submitted values to a wire request and displays the
correlated response. `metis-ui-lang` parses bounded markup and computes a display
list; `metis-platform` rasterizes it into bounded pixel storage. The renderer
implements Iris's lending `RenderBackend<DisplayList>` contract.

Moirai owns execution and process lifecycle. Its transport provider is extended
for inherited private pipes, finite teardown and Windows process-tree lifecycle
containment. Metis retains only application dispatch and the form-session budget.
No frontend object, memory address or backend secret crosses the IPC boundary.

## Provider selection

| Role | Owner | Decision |
| --- | --- | --- |
| Scheduling/process lifecycle | Moirai | Reuse executor and process transport; fill missing pipe/deadline support upstream. |
| Rendering contract | Iris | Implement its borrowed-frame interface; retain byte-packed pixel storage. |
| General allocation | Mnemosyne | Already reachable through Moirai; no extra global allocator override without an allocation contract/measurement. |
| Recoverable framebuffer allocation | Metis | Mnemosyne aligned storage currently aborts on allocation failure; Metis checks size and uses fallible reservation. |
| In-process brand/borrow proofs | Melinoe | Reached through Moirai; cannot replace authenticated serialized capability claims. |
| Standalone MAC/hash | Metis seed implementation | Moirai crypto currently exposes a TLS provider, not a standalone public primitive surface. Upstream extraction remains required before replacement. |

## Platform coverage

The visible desktop event loop and OS permission sandbox remain incomplete.
Process address-space separation and lifecycle containment do not deny file,
network or device access. Platform support claims require separate host tests
and denial probes, not merely `cfg` branches or successful compilation.

See [ADR 0001](adr/0001-process-contract.md) for the trust boundary and alternatives.
