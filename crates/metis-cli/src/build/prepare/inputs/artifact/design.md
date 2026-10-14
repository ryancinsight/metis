# Compiler event dependency ownership

One Cargo `compiler-artifact` event describes one compiler unit and every
reported output of that unit. Generated debug and import/export companions do
not introduce separate source dependency sets. The producer reads every distinct
raw dependency record belonging to primary outputs; it never derives a dependency
filename independently from a companion.

## Pinned producer law

The implementation targets the repository's Rust/Cargo 1.97 toolchain. Its
[compiler unit owner](https://github.com/rust-lang/cargo/blob/rust-1.97.0/src/cargo/core/compiler/mod.rs)
constructs the raw dependency filename from the logical crate name and optional
compiler metadata, then emits all existing output destinations in one event.
The [output type owner](https://github.com/rust-lang/cargo/blob/rust-1.97.0/src/cargo/core/compiler/build_context/target_info.rs)
distinguishes normal/linkable/metadata outputs from auxiliary import/export and
debug outputs. It includes MSVC `.dll.lib`, `.dll.exp`, and `.pdb` companions.

[Cargo's exported dependency writer](https://github.com/rust-lang/cargo/blob/rust-1.97.0/src/cargo/core/compiler/output_depinfo.rs)
writes a separate dependency file beside uplifted primary outputs. That file
contains file dependencies, including transitive path dependencies, but does not
retain rustc's `# env-dep` declarations. Reading only an uplifted primary can
therefore lose environment sensitivity. Every distinct actual raw dependency
record remains authoritative, including `.rmeta`/`.rlib` records.

An uplifted-only unit additionally requires its actual raw build dependency
record. Checked-phase keys cannot substitute: a real procedural macro can
inspect rustc's `--emit` arguments and expand different `env!` calls for metadata
and link emission. The pinned compiler reproduces that counterexample. Compiler
metadata hashes and an intermediate build directory cannot be guessed from an
uplifted filename.

The selected prepared root uses `cargo rustc` with an explicit, Context-owned
`--emit=dep-info=<path>` output. The owned receipt remains attached to compiled
state and matches the exact package, logical target, noncustom role and cdylib
type. No ordinary `.d` is required for that root: the pinned compiler suppresses
its default raw/exported records and recompiles the root on an identical repeat.
This measured cost remains explicit. Ordinary browser builds keep Cargo's normal
build operation and cache behavior.

## Admission

For event `e`, let `R(e)` contain its authoritative raw records. The selected
prepared root has its explicit raw output. A custom build has its dedicated raw
record. Ordinary primary representations retain every distinct associated raw
path. A dylib with reported metadata uses every metadata-associated raw record;
all of its primaries are admitted, but optional exported lists are unnecessary.
An uplifted-only nonroot dylib without reported raw metadata rejects with an
explicit source-shape diagnostic. It cannot borrow checked environment keys or
pretend an exported declaration is raw evidence.

The direct admitted source set is

`files(e) = union(paths(read(d)))` for `d` in `R(e)`.

Environment declarations are the corresponding union of every `# env-dep` key.
The surrounding collector retains its existing package/source, feature,
build-script declaration, directory, payload, and environment ownership rules.
Every compiler-artifact and build-script-executed event requires a package in
the admitted Graph; missing and outside package identities reject. This total
coverage proves the global union includes Cargo's exported lists:
`output_depinfo::add_deps_for_unit` adds raw unit file dependencies, declared
build-script paths, and recursively each local dependency unit. The collector
owns those same raw records and script declarations/generated trees across all
actual units, plus manifests/configuration. Coverage comprises both file records
and declared `DirectoryRecord` scopes with their snapshots and expanded files.
An exported rerun-if-changed directory belongs to that scope abstraction rather
than the flat file set. Every exported dependency is therefore represented by
the complete file/scope union; its exported spelling need not be a file record.
Omitting the redundant exported list cannot hide an environment key.

The event supplies a nonempty logical target name, supported target kinds and
crate types, and nonempty absolute filename strings. Unsupported formats,
malformed events, and events containing only companions reject.

Primary library representations include metadata, Rust libraries, dynamic
libraries, WebAssembly libraries, and static libraries when the declared crate
type permits them. Representation prefixes are removed only after exact
matching against the logical Cargo target name and optional metadata suffix.
The logical name replaces Cargo's target hyphens with underscores. A Windows
crate whose actual name begins with `lib` keeps that name.

Auxiliary `.dll.lib`, `.dll.exp`, and `.dll.a` and debug `.pdb`, `.dSYM`, and
`.dwp` outputs share the compiler unit's primary inputs. They never cause a
search for `name.dll.d` or a debug-output-specific dependency file. Their names
must still belong to the logical target.

A custom-build event owns its reported executable primary and a dedicated
compiler instance directory. That canonical unlinked directory must contain
exactly one canonical unlinked regular `.d` file with the logical unit stem.
The walk streams entries, retains at most one candidate, and admits no more
entries than the existing `FILE_LIMIT`. This bounded lookup does not
scan the shared dependency directory. Missing, linked, or ambiguous dependency
records reject; no failed filesystem operation selects another candidate.

All selected dependency files use Core's `canonical_file` admission, including
the custom-build branch. Reads retain the existing manifest byte bound.
An error retains the package, target, primary output, attempted dependency path,
operation, and original source error. Native error type/kind/code remain in the
source chain rather than becoming a formatted string.

## Verification

The real component fixture compiles an isolated Cargo workspace containing a
build script, a procedural macro whose name starts with `lib`, and a library
with both Rust and dynamic outputs. Real `include_str!` inputs and an `env!`
value prove the computed union and mutation sensitivity. The actual normal
build's exported file paths must be a subset of the subsequent complete raw
input set. This fixture covers file declarations; directory scopes retain their
existing snapshot/expansion tests. A distinct rustc-only environment key cannot pass through build-script
declarations. Native debug/import
companions come from the actual host compiler; Windows vectors run on Windows.

Negative cases cover malformed/companion-only events and missing, linked, and
ambiguous dependency files. Native filesystem errors retain their source chain.
Existing package/input/feature/environment tests and preparation remain required.

The preceding Windows run observes an uncontextualized missing-file error after
successful Cargo check. Its exact failing path is not retained. The companion
contradiction follows independently from actual Windows Cargo JSON and the
pinned producer source; it does not retroactively identify that missing path.
