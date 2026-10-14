"""Admit an existing producer file inside its actual owned directory.

Specification
-------------
For absolute candidate C and directory D, let F = resolve(C, strict=True) and
R = resolve(D, strict=True). Admission requires a regular F, a directory R,
and an A in F.parents for which os.path.samestat(stat(A), stat(R)) holds.
The result is F. Filenames and textual namespace spelling carry no authority.

Proof and ownership
-------------------
Strict resolution removes parent traversals and follows native links before
the finite ancestor chain is inspected. Each parent therefore denotes an
actual directory containing F in that resolved namespace. Equality of native
directory identity proves that one of those ancestors is the owned root.
This accepts Rust's canonical Windows namespace alongside another spelling
of the same root without rewriting prefixes or guessing an inventory name.

The candidate file's identity never proves containment: an outside hardlink
can share an inside file's inode while having no owned ancestor. A resolved
symlink or junction escape likewise has no matching owned ancestor. Links
whose resolved file remains inside the owned root retain their prior policy.
The sole owner below serves preparation inventories and nextest fixtures;
neither consumer retains a second spelling-based admission implementation.

Failure and observation model
-----------------------------
Relative inputs, wrong node kinds and an absent owned ancestor raise
ValueError. Native resolve/stat failures retain their original exception,
errno, filename and text, with an operation/path note; no OS error becomes a
negative containment result. There is one root stat and at most one stat for
each resolved parent. This is a filesystem observation under the caller's
owned output-directory lifecycle, not an atomic capability against external
concurrent namespace replacement.

Python defines samefile/samestat through actual device and inode identities:
https://docs.python.org/3/library/os.path.html#os.path.samestat
PurePath.is_relative_to instead compares strings and is not this contract:
https://docs.python.org/3/library/pathlib.html#pathlib.PurePath.is_relative_to
"""

import os
from pathlib import Path
import stat


def _resolve(path: Path, role: str) -> Path:
    try:
        return path.resolve(strict=True)
    except OSError as error:
        error.add_note(f"native containment resolve {role}: {path!r}")
        raise


def _stat(path: Path, role: str) -> os.stat_result:
    try:
        return path.stat()
    except OSError as error:
        error.add_note(f"native containment stat {role}: {path!r}")
        raise


def contained_file(candidate: Path, directory: Path) -> Path:
    """Return the resolved regular file only when its actual ancestry is owned."""
    for role, path in (("candidate", candidate), ("owned directory", directory)):
        if not path.is_absolute():
            raise ValueError(f"native containment {role} must be absolute: {path!r}")
    resolved = _resolve(candidate, "candidate")
    owned = _resolve(directory, "owned directory")
    if not stat.S_ISREG(_stat(resolved, "candidate").st_mode):
        raise ValueError(f"native containment candidate must be a regular file: {resolved!r}")
    root_stat = _stat(owned, "owned directory")
    if not stat.S_ISDIR(root_stat.st_mode):
        raise ValueError(f"native containment owned directory must be a directory: {owned!r}")
    for ancestor in resolved.parents:
        if os.path.samestat(_stat(ancestor, "ancestor"), root_stat):
            return resolved
    raise ValueError(f"native file is outside its owned directory: {resolved!r}; owner={owned!r}")
