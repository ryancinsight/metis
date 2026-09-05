# Risk controls and open evidence

| Hazard | Implemented control | Evidence or unresolved requirement |
| --- | --- | --- |
| Unauthenticated calculation request | Backend-held key, scoped session token, exact issued-token binding, real clock and monotonic lifetime | Token tamper, cross-session, expiry and rollback regressions. |
| Misattributed/replayed response | Strict request order and response correlation | Wire replay and wrong-sequence tests. |
| Invalid numeric policy disables limits | Private validating envelope fields | Nonfinite, nonpositive, boundary and exceeded-limit tests. |
| Unbounded hostile markup/frames | Explicit byte/depth/node/surface/queue limits | Malformed corpus and limit regressions. |
| False authenticity claim in display | UI states backend MAC is not frontend-verified | The frontend has no signing key and cannot verify a symmetric backend MAC. |
| Audit ambiguity or secret logging | Fixed typed records and canonical hashing; no raw patient text in audit | In-memory integrity tests; durable recovery/trusted checkpoint outstanding. |
| Frontend has host privileges | Separate executable contains no clinical backend dependency | OS permission sandbox and native denial probes outstanding. |
| Web content obtains native authority | Target: deny-by-default commands, origin/session binding, bounded asynchronous bridge | Browser/WebView host and hostile-origin, navigation, injection and teardown probes outstanding; see [ADR 0002](adr/0002-web-application-contract.md). |
| WASM or Rust mistaken for comparative security/memory evidence | Claims distinguish portable compilation from host enforcement and measurement | Matched Tauri process-memory measurements and comparative threat tests outstanding. |
| Backend freeze during IO | Bounded supervised frontend lifecycle | Process tests; arbitrary application handler computation remains synchronous and must itself be bounded. |
| Regulatory assurance inferred from a demo | Claims restricted to observed engineering evidence | No completed clinical validation, risk-management file or regulatory submission exists. |

Cryptographic vectors establish agreement for tested inputs. They do not establish
constant-time machine code, cryptographic-module validation or resistance to an
attacker controlling the backend process. The hash chain alone cannot prevent
an attacker rewriting the entire ledger without an external trusted checkpoint.
