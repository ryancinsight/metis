"""Check Metis distribution identity and retain a verifiable upload receipt."""

import argparse
import email.parser
import hashlib
import json
import pathlib
import re
import tarfile
import urllib.parse
import urllib.request
import zipfile

from packaging.version import Version


PROJECT = "metis-ui"
REPOSITORY = "ryancinsight/metis"


def metadata(path):
    """Read the single distribution metadata document without extracting files."""
    if path.name.endswith(".whl"):
        with zipfile.ZipFile(path) as archive:
            names = [name for name in archive.namelist() if name.endswith(".dist-info/METADATA")]
            if len(names) != 1:
                raise ValueError(f"{path.name}: expected one wheel metadata document")
            raw = archive.read(names[0])
    else:
        with tarfile.open(path, "r:gz") as archive:
            # A source archive can include dependency PKG-INFO files. Only the
            # root distribution's metadata establishes the upload identity.
            members = [
                member for member in archive.getmembers()
                if member.isfile() and len(pathlib.PurePosixPath(member.name).parts) == 2
                and pathlib.PurePosixPath(member.name).name == "PKG-INFO"
            ]
            if len(members) != 1:
                raise ValueError(f"{path.name}: expected one root source metadata document")
            with archive.extractfile(members[0]) as source:
                raw = source.read()
    return email.parser.BytesParser().parsebytes(raw)


def normalize(name):
    """Apply the Python project-name comparison rule."""
    return re.sub(r"[-_.]+", "-", name).lower()


def python_version(cargo_version):
    """Use the PEP 440 conversion Maturin applies to Cargo's version string."""
    # Maturin1.14.1 parses package.version.to_string() with pep440_rs::Version
    # (src/metadata.rs). PyPA's parser implements the same specification.
    return str(Version(cargo_version))


def create_receipt(directory, version, commit):
    """Reject unrelated artifacts before upload and hash the validated file set."""
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("source commit must be a full Git SHA")
    expected_version = python_version(version)
    paths = sorted(directory.iterdir())
    if any(not path.is_file() or not path.name.endswith((".whl", ".tar.gz")) for path in paths):
        raise ValueError("distribution directory contains an unexpected entry")
    if sum(path.name.endswith(".tar.gz") for path in paths) != 1:
        raise ValueError("expected exactly one source distribution")
    if not any(path.name.endswith(".whl") for path in paths):
        raise ValueError("expected at least one wheel")
    files = {}
    for path in paths:
        fields = metadata(path)
        names, versions = fields.get_all("Name", []), fields.get_all("Version", [])
        if len(names) != 1 or normalize(names[0]) != PROJECT:
            raise ValueError(f"{path.name}: distribution name does not match {PROJECT}")
        if versions != [expected_version]:
            raise ValueError(f"{path.name}: distribution version does not match {expected_version}")
        files[path.name] = {"sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "size": path.stat().st_size}
    return {
        "schema": 1,
        "project": PROJECT,
        "version": expected_version,
        "cargo_version": version,
        "source": {"repository": REPOSITORY, "commit": commit, "workflow": "python-release.yml"},
        "files": files,
    }


def verify_index(receipt, published):
    """Require PyPI to report the same version, filenames, hashes and sizes."""
    if normalize(published["info"]["name"]) != receipt["project"]:
        raise ValueError("PyPI project identity differs from the built distribution")
    if published["info"]["version"] != receipt["version"]:
        raise ValueError("PyPI version differs from the built distribution")
    actual = {row["filename"]: row for row in published["urls"]}
    if len(actual) != len(published["urls"]) or actual.keys() != receipt["files"].keys():
        raise ValueError("PyPI file set differs from the validated upload")
    for name, expected in receipt["files"].items():
        row = actual[name]
        if row.get("yanked") or row["digests"]["sha256"] != expected["sha256"] or row["size"] != expected["size"]:
            raise ValueError(f"{name}: PyPI checksum, size or availability differs from the validated upload")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("create")
    create.add_argument("--dist", type=pathlib.Path, required=True)
    create.add_argument("--version", required=True)
    create.add_argument("--commit", required=True)
    create.add_argument("--output", type=pathlib.Path, required=True)
    verify = commands.add_parser("verify-index")
    verify.add_argument("--receipt", type=pathlib.Path, required=True)
    verify.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()
    if args.command == "create":
        result = create_receipt(args.dist, args.version, args.commit)
    else:
        receipt = json.loads(args.receipt.read_text(encoding="utf-8"))
        if receipt["project"] != PROJECT or not receipt["version"]:
            raise ValueError("receipt must name the intended Metis release")
        version_path = urllib.parse.quote(receipt["version"], safe="")
        url = f"https://pypi.org/pypi/{PROJECT}/{version_path}/json"
        with urllib.request.urlopen(url, timeout=20) as response:
            published = json.load(response)
        verify_index(receipt, published)
        result = {"receipt": receipt, "index_url": url, "index": published}
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"{PROJECT}: {args.command} verified; receipt: {args.output}")


if __name__ == "__main__":
    main()
