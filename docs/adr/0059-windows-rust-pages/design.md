# Windows Rust page domains and acceptance contract

Owner: [ADR 0059](../0059-windows-rust-pages.md).

State: implementation under verification; real Windows acceptance pending.

## Dependency graph

```text
metis CLI (native build orchestration)
  -> metis-core::embedded_assets (inventory, codec, integrity)
  -> Cargo/wasm-bindgen subprocesses with bounded lifetime

WASM artifact: metis-web::embedded
  -> metis-web::Theme
  -> metis-frontend::{ApplicationCommand, CommandMenuState}
  -> metis-core, metis-ipc (existing presentation dependencies)
  -> moirai-pal::wasm (DOM, bridge, permissions, local tasks/timers)

metis-app build.rs
  -> metis-core::embedded_assets
  -> verified prepared artifacts -> owned OUT_DIR embedding

metis-app (Windows native process)
  -> metis-web::embedded (shared codec/domain messages; private JSON DTOs)
  -> metis-platform -> moirai-pal::windows::webview
  -> metis-frontend -> private metis-ipc -> supervised metis-backend
```

The WASM and frontend dependency closures contain no `metis-app`,
`metis-backend` or `metis-cli`. The CLI and build script do not depend on one
another. The core integrity module imports no presentation or distribution
crate. Its SHA-256 implementation uses the existing Moirai crypto dependency.
Serde belongs to declared JSON boundaries in `metis-app`, `metis-web` and
the CLI, plus the core integrity boundary that parses complete Cargo lock/config
TOML. Update the registry-boundary gate with that scoped rationale.

Use the existing `metis-web` cdylib, with Rust start/stop exports for the embedded
page. The HTML role selects form or permission probe in Rust. No parallel crate
copies the command, theme or form algorithm. Measure generated artifact size
and compile/runtime cost; remove unreachable work through normal release
optimization without weakening the behavior or acceptance scenarios.

### Application configuration root

The browser configuration is `.config/browser/metis.json`, outside the served
frontend. Its explicit `application_root` is `../..`. Both closed application
shapes share the same root resolver. Omission means the canonical manifest
directory; an explicit locator has at most the canonical 240-byte path budget,
portable forward-slash components and explicit parent components. `.` is valid
only as the whole locator. Absolute paths, drive prefixes, empty components,
backslashes and linked directory traversal are rejected. Resolution must produce
an existing directory that contains the canonical manifest; the manifest cannot
select an unrelated sibling tree. Every Cargo, frontend, icon and resource path
still uses its existing strict relative-path contract against the selected root.
There is no implicit discovery or alternate-root retry. Real filesystem tests
cover omitted/explicit roots, containment, links and malformed locators, followed
by a real browser build from the relocated manifest.

A development session owns one root `R`: its watcher, snapshot and every launched
build use `R`. Native watch reloads reject a manifest resolving to `R' != R`
before compiling or launching it, and continue observing `R`. Selecting another
root requires restarting the command. The browser session likewise retains its
initial parsed configuration. A root transition cannot leave a live watcher
bound to a different application tree.

## PAL mechanisms

PAL keeps its existing public WASM entry module: `WebDocument` and related DOM
types already export through `moirai_pal::wasm`. New bridge/permission types
export there from private domain leaves, without a second public spelling for
an existing API. `wasm/webview` owns the WebView2 bridge, obtained from the actual
current window. It exposes bounded JSON posting and an owned message listener.
Posting
parses the supplied JSON and passes its object value to `chrome.webview`;
posting the serialization as a string changes `WebMessageAsJson` into a quoted
string. Receiving accepts actual object data or a string containing JSON,
returns bounded JSON to the consumer and preserves browser error context.
Missing bridge, malformed data, byte overflow and browser exceptions remain
distinct errors. Explicit close detaches the listener before releasing the Rust
closure. A removal exception retains its counted realm owner and reports failure. The provider imports no Metis schema or permission policy.
[Microsoft defines the object and listener contract](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/javascript/webview).

The native and WASM bridge share one PAL limits definition, including the
existing 64 KiB JSON byte/unit bounds and 256-event capacity. Move its definition
to a common private provider domain and update internal native callers. Existing
native public names keep their established export location; the WASM bridge
enforces the same limits internally, without new synonyms for those constants.
Enforce bounds before retained allocation and
after actual JSON serialization. No new selector or synthetic-click API is
needed: the page's command IDs are fixed and click/shortcut handlers apply the
same Rust command transition.

`wasm::permissions` separates geolocation, media and notification WebIDL domains.
Return real typed unavailable/rejected/granted outcomes with native error
code/name context. Camera and microphone use one media request API with a
constraint kind, rather than copied request implementations. Notification
results retain all three browser values: granted, denied and default.
No API returns a fabricated denial or changes the native host's deny policy.

The geolocation mechanism owns a `watchPosition` registration and resolves on
its first actual position or error. It calls `clearWatch` on completion,
deadline and drop while realm dispatchers retain queued callback validity. This replaces the uncancellable
one-shot registration while preserving this probe's single-outcome behavior;
the page neither stores nor publishes coordinates. Callback state must reject
late delivery after cancellation. A browser position timeout alone does not
bound permission waiting, so a PAL timer also owns the operation deadline.
[Geolocation defines watch cancellation and permission waiting](https://www.w3.org/TR/geolocation/#watchposition-method).

Media uses real `getUserMedia` constraints `{video: true}` or `{audio: true}`.
A successful stream becomes an owned lease whose drop stops every track. The
Promise completion handler owns cleanup before delivery to the consumer; if
the consumer cancels before acquisition, late success immediately drops the
lease. Dropping a future does not abort this browser Promise. Retain only the
provider settlement state needed for cleanup, without retaining the page DOM.
Admit at most one unsettled request for each capture kind per document; a new
request returns a contextual busy error until settlement. Thus repeated remount
cannot accumulate pending capture requests. Report retained settlement guards
separately from active application tasks; never report them as already freed.
[Media Capture defines the request and track lifecycle](https://www.w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia).

Notifications call real `requestPermission`. Their Promise also has no abort
parameter. Cancel publication, retain bounded settlement ownership and reject
another unsettled notification request in the same document. Release settled
handlers; document destruction terminates the remaining browser realm. Do not
equate an ignored or default result with denial.
[The Notifications standard specifies the returned permission](https://notifications.spec.whatwg.org/#dom-notification-requestpermission).

All deadline waits use the minimum of the caller's remaining deadline and the
existing provider 30-second operation bound. Page stop invalidates its current ready mount
before dropping listener/task handles; remount advances the mount identity. A completion publishes only
when its mount and request generations still match. Generation checks prevent
stale DOM writes; actual watch/track/Promise ownership performs resource cleanup.

## Page behavior and protocol

`ErrorCode` retains its established fieldless `repr(u16)` representation.
Derived `bytemuck::NoUninit` proves that valid enum values contain no padding or
uninitialized bytes. Const `must_cast` proves source/target size equality;
`u16` admits every target bit pattern. The native representation therefore maps
to the identical discriminant without a numeric cast, lookup, allocation or
runtime layout check. The inverse decoder retains its unknown-code rejection.
Exhaustive wire-domain tests and optimized-code comparison verify this contract.

The private `metis-web::embedded::wire` domain owns serialized request and
response DTOs. Public codec operations expose typed page-domain messages to
the native consumer, rather than exposing Serde DTOs or introducing a second
schema definition. The new inventory and required WASM/codec entry points are
public additions; the native process's request-handling DTOs are not public API.
Use a versioned closed schema, reject unknown actions/fields, and apply the same
64 KiB bounds on both sides. `PageRole` is form or permission-probe. Mount and
request identifiers are nonzero, monotonically advancing `u64` values encoded
as canonical decimal strings. At most one calculation is outstanding.
Native replies carry the admitted request/mount identifiers; stale, duplicate
or mismatched replies cannot update output or release another request's button.

The native host supplies the schema version, selected role, expected mount
identifier and module inventory digest in packaged configuration. `PageReady`
echoes those values only after Rust resolves every required DOM element,
applies the initial theme, installs listeners and acquires the real bridge.
The host validates the provider-observed source, exact configuration and state
transition from Loading to Ready. Navigation completion or an arbitrary native
event batch is insufficient. Duplicate/stale readiness is rejected.

Readiness is not proof of a trusted gesture or a permission grant. The native
source policy, CSP, page schema and backend authorization remain separate.
First-frame/theme capture waits for Ready. A missing/failed module, unavailable
bridge, malformed ready message or deadline causes an explicit failed capture;
it never emits an accepted blank-frame record.

The native lifecycle is Loading -> Ready -> Stopping -> Stopped -> Closed. A Rust page
coordinator owns the bridge while the mounted form/probe owns DOM listeners and
tasks. Native `StopPage` addresses the exact ready mount. Rust invalidates that
mount before dropping its application handles, then sends `PageStopped` with
the stopped mount, schema/role/module identity and computed resource counts.
The coordinator retains its bridge and the stop-publication future required
to deliver that acknowledgement and a later native mount command. An acknowledgement does not hide unsettled
Promise guards. The host accepts it once for that Stopping mount, closes the
actual controller and observes native destruction before recording Closed.
Deadline or teardown error closes the controller and marks the handshake failed.
For remount, the host supplies a strictly greater nonzero identifier after the
previous mount's stop acknowledgement; only a new matching Ready admits work.
Request counters never reset within a mount, and wraparound rejects rather
than reusing an identifier. Close/Escape use this same bounded stop handshake.

Preserve these computed behaviors:

| Domain | Contract |
| --- | --- |
| Initial theme | Recognized query value precedes body configuration; otherwise use the authored selected theme |
| Theme change | Select value, body attribute and `Theme: ...` text agree for system/light/dark/high-contrast |
| Menu toggle | ARIA expanded/hidden and open attribute agree; status is `Commands opened` or `Commands closed` |
| Menu focus | Opening focuses dark; closing focuses toggle only when it was previously open |
| Patient command | Close menu, then focus patient; status is `Patient reference focused` |
| Theme commands | Dark renders `Theme: dark`; system renders `Theme: system preference` |
| Escape | Open menu closes and consumes Escape; host-level Escape otherwise exits |
| Shortcuts | Physical `Key[A-Z]`, exact Control/Alt/Shift/Meta modifiers and no repeated activation |
| Submit | Prevent default, disable submit and show the existing supervised-backend submitting message |
| Patient input | Preserve Unicode; host rejects empty, control-containing or over-128-byte identifiers |
| Numeric input | Preserve empty-value conversion to zero and finite browser number semantics; backend remains range authority |
| Reply | Show actual rate, drug rate and audit ID; preserve contextual error status and four-digit hexadecimal code |
| Terminal failure | Release the current submit state after a typed matching error; malformed bridge data reports its cause |
| Missing bridge | Explicit unavailable host/result text; no synthetic backend result |

The query reader bounds decoded values by the longest declared theme name.
PAL's only `InvalidInput` outcome for this operation means that value exceeds
the bound; it therefore cannot belong to the finite theme domain. Treat that
outcome as an unknown query and preserve the authored theme. Preserve every
other observation error, including failure to obtain the current location.

The existing browser controller's produced-key fallback and different menu/system
labels do not replace the native contracts. Share the command catalog and state,
then apply the selected view's text/focus policy. For number controls, browser
value sanitization and empty conversion are explicit cases; Rust parse failure
does not silently introduce a different range-rejection path. Reject nonfinite
values with contextual failure before serialization.
[HTML specifies number-input sanitization](https://html.spec.whatwg.org/multipage/input.html#number-state-(type=number)).

`audit_sequence_id` also uses canonical decimal-string encoding, decoded as
`u64` in Rust, without sign, whitespace, leading zeros or overflow. Preserve
backend floating-point rates as their existing finite binary64 values. Native
`PostWebMessageAsJson` creates JavaScript object numbers, so a numeric `u64`
field cannot retain every audit identifier; do not cap or round the domain.
[Microsoft specifies object parsing](https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2.postwebmessageasjson),
[ECMAScript specifies the safe integer bound](https://tc39.es/ecma262/2023/multipage/numbers-and-dates.html#sec-number.max_safe_integer).

The probe calls geolocation, camera, microphone and notifications in that order.
Each row records the actual browser outcome. A matching native permission-denied
event overwrites the browser row and retains its user-initiated flag; a later
browser rejection cannot replace that host denial. Completion carries four
typed outcomes, not only a count. The native host independently records its
permission events. Capture waits for readiness and four settled outcomes;
denial acceptance additionally requires all four actual native kinds and no
granted outcome. Unavailable APIs or missing media hardware cannot masquerade
as host denial. A failed/deadline probe produces a failed evidence record.

## Prepared assets and coherence

Add an optional native `embedded_module` declaration with package `metis-web`
and platform `windows`. Omission preserves native applications without this
page. Presence does not change `Manifest::read` into the browser shape: only
the existing `frontend` member does that. Reject mixed/unknown fields and an
embedded module whose package is not a workspace cdylib owner. Existing native
and browser callers migrate in the same release.

Factor the existing CLI WASM compiler/artifact selection and wasm-bindgen
generation into their canonical build domain. Both browser packaging and native
preparation call it. Select the actual compiler-reported artifact; never infer
a target-directory filename. Require locked Cargo resolution and exactly one
matching wasm-bindgen crate/CLI version. Native `metis build`/`package` prepare
the module before compiling the executable. Add `metis prepare` for the same
native declaration so direct Windows developer/build/test workflows obtain the
same inventory before Cargo. No alternate script owns generation policy.

The typed `EmbeddedModuleInventory` lives under core's existing integrity/wire
boundary. Its canonical binary encoding avoids adding JSON dependencies to
core or a build-script dependency on the CLI. Keep the existing readable JSON
reports, derived from decoded values, with sizes and hexadecimal SHA-256 values.
The binary format is versioned, documented and tested; it is not an opaque blob.

Encoding order is: eight ASCII magic bytes `METISAS1`, big-endian schema `u16`,
module package/target identity, locked-graph digest, tool identities, build
configuration, directory-membership records, source-input records and generated-asset records. Strings use
big-endian `u32` byte lengths followed by valid UTF-8; records use explicit
counts, `u64` byte lengths and 32 SHA-256 bytes. Reject unknown schema, truncation,
trailing bytes, integer overflow, duplicate/case-colliding paths and unsorted
records. Canonical owner and artifact identifiers omit host-absolute roots and
timestamps. Effective compiler settings retain literal values, including paths
when they are declared inputs. Local owner/tool bindings remain in the sidecar;
rebinding roots alone preserves identity, while changed compiler settings may
change it.

Each sequence count is big-endian `u32`; each enum tag is one validated byte.
After magic/schema, field order is fixed: package name, Cargo package ID, cdylib
target, WASM target, export name, native platform tag, page schema, lock digest,
provider identities, three tool records (Cargo/rustc/wasm-bindgen), build
configuration, directory records, source-input records, then two asset records. A provider record
contains package ID, package name, version and source kind followed by its Git URL/revision
or registry checksum; path owners must be workspace members. A tool record
contains its version output and resolved executable digest. Configuration
contains profile, sorted unique features, ordered compiler arguments and sorted
unique environment keys with optional values; one closed presence byte distinguishes
unset from an explicitly empty value. Directory records precede input records
and retain owner, optional relative directory, admission policy and exact sorted
membership, including empty directories. Input records contain owner ID, relative
path, length and digest; asset records contain JavaScript/WASM kind, relative
destination, length and digest. Sort records by owner/path or destination, not
compiler arguments. The core codec is the only encoder/decoder definition.

The contract records:

- `wasm32-unknown-unknown`, selected Cargo package ID/cdylib target and declared
  export, plus native consumer platform and page schema version.
- Exact Cargo.lock SHA-256 and canonical provider source identities.
- The pinned compiler version and locked matching wasm-bindgen version required
  by the existing gate, plus observed Cargo version and resolved tool executable
  SHA-256 values for provenance/stale detection. These do not introduce a tool
  hash allowlist or trust policy beyond canonical tool/version admission.
- Release profile, feature set, effective Cargo configuration, target and
  compiler flags/environment that affect the artifact.
- Every compiler/build-script-declared file input in the resolved module
  closure, including manifests, Rust source, embedded files and generated
  inputs, with owner namespace, normalized relative path, size and SHA-256.
- Exactly the generated JavaScript module and WASM module, their canonical
  destination names, sizes and SHA-256 values; WASM magic/version validation.

An input namespace identifies its workspace or locked dependency owner; path
resolution stays inside that owner's canonical root. Reuse the existing
240-byte portable path rule, 4096-record count and 1 MiB manifest budget as
shared core contracts, updating their previous CLI owners and every caller.
Generated modules reuse the existing 64 MiB per-module admission budget;
checked addition must also obey the native 1 GiB payload bound. Stream hashes
and check file metadata before retained allocation. Decode checks lengths/counts
before reservation and uses checked arithmetic. Reject links/reparse points,
absolute paths, traversal, reserved names and case collisions for all inputs.
No build script may read undeclared inputs; a missing recorded input is a
coherence failure rather than a reusable cache entry.

The CLI fingerprints inputs before and after module generation and native
compilation; a changed input invalidates the operation. It supplies the
absolute prepared inventory path through `METIS_WEBVIEW_ASSET_INVENTORY` only
to the native compile. The application build script decodes and independently
checks current inputs, lock/config/tool identities, schema and artifact bytes,
then copies those verified bytes into its owned OUT_DIR before embedding.
It declares rerun dependencies for the inventory, inputs, artifacts and relevant
environment. The Rust compiler embeds that owned copy, not an independently
re-read external pathname. Missing/stale/mismatched preparation fails with its
specific cause and preparation command; no empty or older snapshot is selected.

`PreparedModule::open` selects one sealed zero-sized context policy. Both
`ProducerContext` and `NativeContext` admit the same current lock, input,
configuration-file, directory, bound-tool and asset identities. The producer
also compares its current effective WASM environment and tool selectors before
and after compilation. Native Cargo's derived flags and Clippy wrapper describe
the native compile; `NativeContext` consumes the recorded typed WASM settings
instead of interpreting those native settings as new module inputs. Unset and
empty producer values remain distinct. Neither policy bypasses source or asset
admission, and both use the same non-generic IO implementation.

The installed executable retains embedded bytes and their inventory identity.
The runtime package writes index, CSS, the initialization-only bootstrap,
generated JavaScript and WASM into an exclusively owned temporary directory.
It verifies extracted bytes and cleans up after closing the controller; partial
write/cleanup errors retain both contexts. Preserve eight create-new attempts.
Only `https://metis.invalid/` maps this directory. CSP permits
`script-src 'self' 'wasm-unsafe-eval'` and `connect-src 'self'` for module loading;
retain remote, inline-script, eval, worker, frame, form, host-object and ordinary
network denials. Mapping does not add backend authority.
[Microsoft documents virtual-host local content](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/working-with-local-content).

## Verification and delivery order

1. Finish the independent 0.4.1 dependency/capture-bound patch. Implement PAL
   bridge/capability ownership from this specification, with portable state/bound
   tests, WASM diagnostics and real visible WebView2 integration. Publish its
   canonical provider identity before changing Metis locks.
2. Implement the shared inventory and codec, canonical CLI preparation stage and
   build-script verification. Test computed round trips/hashes, changed source,
   wrong tool/lock/config/schema, every truncation boundary, oversize lengths,
   traversal, links and artifact corruption. CLI JSON and embedded values agree.
3. Implement the Rust page/protocol with value-semantic command/theme/focus,
   numeric and denial-reconciliation tests. Exercise all `u64` boundaries,
   including values around `2^53` and `u64::MAX`, malformed/duplicate replies and
   stale generations. Replace script-substring assertions with these contracts;
   retain every production restriction and native integration scenario.
4. Migrate packaging, native source/configuration and capture readiness. Update
   Windows direct Cargo, Clippy/test, CLI dev/build/package, distribution and
   release callers to prepare assets before native compilation. Audit raw
   embedded scripts and all product loaders: authored JS performs initialization
   only, with zero application behavior remaining from the 213-line controllers.
5. Add `scripts/webview_acceptance.py` as the canonical real Windows orchestrator
   and call it from `scripts/verify.py` after the prepared native build. Keep
   scenario/trace domains in leaf modules below 500 lines. The existing Windows
   `ci.yml` gate runs this stage with a strict 60-second timeout and uploads its
   bounded trace/PNG artifacts through the existing evidence mechanism.
6. The Windows stage preflights installed WebView2, visible HWND/controller,
   interactive input access and all four API/request paths. Missing prerequisites
   fail the Windows acceptance stage; they are not skipped or inferred from a
   screenshot. Continue the owner-thread message pump during waits/capture.
   Record runtime/tool/source/lock/module/executable hashes and actual readiness,
   source-rejection, result and permission events. No devices or interactive
   Windows runtime are established by this macOS design review.
7. Use real SendInput/UI Automation to verify changed numeric/Unicode patient
   inputs and analytical backend rates, error recovery, Return/click submission,
   themes, menu focus, physical shortcuts, resize and close. Send the actual
   native mount/stop protocol through WebView2 for lifecycle scenarios;
   no script fills results, synthesizes denied events or substitutes browser
   delivery. Assert callback/task/watch/track counts and bounded outstanding
   settlement guards through remount and teardown. An unexpected grant fails.
8. Compare the old and new Windows journeys against the same declared behavior
   and independent rate equations; accept equivalent computed/DOM outcomes, not
   merely changed pixels. Preserve the separate framebuffer, trusted-input,
   installed-IME and multimonitor evidence scopes. Run all existing Rust/script
   suites, native/WASM warning-denied checks and minor-class compatibility gates.
   Synchronize versions, changelog, manuals, inventory examples, backlog, gap
   audit, ADR index and canonical locks only with truthful implementation evidence.

The selected increment is complete only when its real Windows stage passes.
Unavailable media hardware, blocked interactive input, provider refusal, timeout
or an unobserved permission kind remains a failed acceptance condition. Compilation
on macOS, a completion count or historical native captures cannot close it.

## Source-derived ownership refinements

- The page preserves the backend's 32-byte result signature as well as its rates
  and exact audit identifier. `CloseRequested` carries the admitted identity.
  A valid stop acknowledgement advances to `Stopped`; only observed native
  destruction advances to `Closed`. The coordinator's stop future remains
  separate from application futures; counters observe actual child-future drop.
- `clearWatch` can leave queued callbacks, and a cancelled cleared watch may
  never settle. Two realm-owned geolocation dispatchers retain callback validity;
  at most four unsettled guards retain one admission slot per actual kind.
  Stop reports dispatchers and guards without claiming their destruction.
- Application DOM listeners have explicit fallible close. Actual listener
  removal failures retain counted callback owners, prevent further admission,
  and reject successful stop acknowledgement. Native bridge cleanup follows
  the corresponding one-owner law.
- A bounded local `bindings.bin` sidecar maps semantic owner/tool identities to
  actual canonical roots. Those binding paths remain outside inventory identity;
  explicitly supplied compiler configuration preserves its literal path values.
  Generated inputs use the locked provider's actual build-output namespace.
  Declared directory scopes preserve every member; default source scopes omit
  Cargo/Git-owned metadata according to their explicit admission policy.
- Native preparation fingerprints the source closure before Cargo, runs locked
  release checking to discover actual build-script/generated inputs, then
  fingerprints the reported inputs before release code generation. Build and
  generation must preserve those identities and directory memberships. Native
  compilation independently verifies the resulting inventory.
  All effective Cargo codegen/configuration environment settings enter the
  configuration identity, with unset and empty values distinguished.
  The [typed root receipt](../../../crates/metis-cli/src/build/module/receipt.md)
  retains actual raw code-generation inputs; exported dependency lists omit
  environment declarations. Explicit root emission recompiles that preparation
  root, while ordinary browser builds retain normal Cargo caching.
- Tool execution uses `ProcessSpec::working_directory` for child-only neutral
  configuration discovery. It never mutates the parent's directory. Direct
  Cargo tool resolution verifies the actual selected rustup compiler, including
  executable hash/version; unresolvable wrappers fail with their context.
- Native content and the empty owned browser profile are sibling directories.
  WebView2's actual environment user-data folder must match the requested
  canonical profile. Every real denial sets `DENY` and disables profile saving
  before observation. Explicit profile close waits for the actual browser PID's
  `BrowserProcessExited` event before the caller removes its directories.
- Decimal UI formatting uses PAL's actual ECMAScript Number boundary. The JSON
  codec enables exact float round trips; a finite IEEE-754 exponent/sign/mantissa
  boundary matrix rejects parser rounding drift without a tolerance adjustment.

The permission page publishes computed settled outcomes, then one final
reconciliation when delayed actual host-denial commands fill every observation.
Repeated callbacks do not produce further final-completion events. Native
denial acceptance independently checks its real event set and the reconciled
page observations. Synchronous native frontend/backend exchange retains its
existing execution contract; continuous event pumping within that exchange
requires a separately specified bounded/cancellable IPC increment.

The canonical observation predicate accepts geolocation rejection only with
the specified permission-denied code 1, media rejection only with the specified
`NotAllowedError` name, and notification denied/default only in that domain.
Reconciliation additionally requires the independently observed native denial.
Unavailable devices, acquisition errors and timeouts cannot establish denial;
native admission rejects them with their original browser context. A genuine
native denial and permission error do not prove successful device acquisition.
[Geolocation defines its error codes](https://www.w3.org/TR/geolocation/#position_error_interface);
[Media Capture distinguishes permission and device failures](https://www.w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia).

Finite binary64 fields use canonical shortest decimal strings in the private
schema. JSON object serialization erases numeric negative zero; text preserves
its sign and the exact parser preserves every finite bit pattern. The bound is
24 bytes from 17 significant digits and the binary64 exponent domain. Numeric
JSON fields, noncanonical spellings and nonfinite values fail admission.

Preparation records the effective Cargo configuration, including ordered include
precedence and `[env]` force/relative semantics. Producer and consumer use one
core computation. Unresolved relative executable paths, absent optional includes,
and unsupported legacy configuration include origins fail with context. They do
not select an alternative tool or ignore an admitted input. Short and
host-qualified Rustup channel spellings identify the same tool only after actual
resolved executable/version verification against the repository pin.
Rustup discovery preserves its absolute invocation alias: a multicall image
dispatches by that name. Proxy recognition hashes the canonical image; returned
Cargo/rustc bindings still identify the canonical selected tool executable.

The coherence contract assumes exclusive ownership of trusted source and
generated output during preparation and compilation. Endpoint fingerprints
detect changed inputs; they do not attest hostile change-and-restore activity.
Tool provenance covers resolved executable bytes, version and declared compiler
inputs. It does not attest precompiled sysroot libraries or compiler-driver
dynamic libraries. No full-host or full-toolchain attestation is claimed.

Remount initialization restores closed menu metadata, enables submission and
clears the previous request presentation. Patient draft values and the selected
theme survive because the document remains owned by the coordinator.
