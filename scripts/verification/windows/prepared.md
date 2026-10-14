# Original native preparation evidence

The Windows gate places the actual `metis prepare` output beneath
`output/browser/windows-webview-prepared/<run nonce>`. The existing CI upload
includes this directory. It is outside the CLI's packaged browser application
tree and the native page's extracted content/profile trees. Neither
verification nor the artifact uploader reconstructs,
copies, renames, or substitutes its inventory, local bindings, JavaScript,
or WebAssembly bytes.

## Contract

Given the CLI-reported inventory `I`, its exclusively owned output directory
`D`, and the same release CLI's `asset-budgets` value `B`, retention requires:

1. `I` is an actual regular file contained by directory identity in `D`.
2. Every enumerated node in `D` is an unlinked regular file or directory.
3. The complete enumerated descendant-node count, excluding `D` itself, is at
   most `B.records`. Each relative UTF-8 name has at most
   `B.portable_path_bytes` bytes. Each file has at most
   `B.module_bytes` actual bytes, and their sum is at most `B.payload_bytes`.
4. Exactly one enumerated file has `I`'s actual native file identity.
5. The receipt records each relative filename, actual byte count and SHA-256,
   plus the original reported inventory path and its matched retained name.
   Its UTF-8 representation fits `B.manifest_bytes`.

Enumeration counts directory nodes as well as files, so directory breadth
and depth cannot evade the canonical record bound. A streaming hash counts
the bytes it reads and stops on a bound violation; it does not rely on the
initial file length. Its buffer is bounded by the canonical manifest/module
limits. Sorting the bounded final file records gives deterministic ordering.

Directory entries supply names only. Each descendant's identity, type and
reparse attributes come from a fresh `Path.stat(follow_symlinks=False)` call.
That is the non-following native stat operation on every supported host;
`os.path.samestat` compares its actual device/file identifiers with the
inventory's same native metadata domain. CPython's Windows `DirEntry.stat`
cache omits identity fields, so enumeration metadata cannot establish this
equality. The [pinned CPython contract](https://github.com/python/cpython/blob/v3.13.15/Doc/library/os.rst)
requires `os.stat` to obtain those fields; the [implementation](https://github.com/python/cpython/blob/v3.13.15/Modules/posixmodule.c)
uses cached `win32_lstat` for `DirEntry.stat(follow_symlinks=False)`.

The receipt is created exclusively beside `D`, never inside it. Thus recording
evidence does not modify the producer's membership or bytes, and a repeated
receipt cannot overwrite an earlier record. The original output remains in
the upload subtree even when receipt admission rejects. Native I/O errors
retain their exception type, source values and an operation/path note.

## Authority and limits

Python treats every producer file as opaque bytes. It does not implement a
second inventory decoder, assign guessed JavaScript/WASM names, or validate
compiler provenance. Rust's `PreparedModule<NativeContext>` remains the
authority for the inventory, tools, source closure, generated asset roles and
digests. Native acceptance independently records the inventory SHA-256. The
receipt makes the exact producer bytes available for that comparison; it
does not establish successful page startup or any browser outcome.

This is a filesystem observation during the gate's exclusively owned output
lifecycle, not an atomic defense against concurrent external replacement.
Existing deadlines, native admission, workloads and upload permissions stay
in their owning components. Focused tests use real filesystem objects and
analytical opaque byte vectors; those vectors do not claim to be valid WASM
or a valid semantic inventory. The actual producer replay verifies integration.
