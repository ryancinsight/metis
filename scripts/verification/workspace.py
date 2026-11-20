"""Workspace executable ownership and direct dependency contracts."""

import pathlib
import tomllib


def native_build_directory(metadata, host, environment, *, explicit_target):
    """Select the native artifact directory shared by verification and packaging."""
    directory = pathlib.Path(metadata["target_directory"])
    if explicit_target:
        # Windows packaging already selects this host triple explicitly.
        # Cargo and Maturin must use that same identity earlier in the gate;
        # an implicit host build otherwise recompiles into a different tree.
        environment["CARGO_BUILD_TARGET"] = host
        directory /= host
    return directory


def pull_request_packages(metadata, root, changed_paths):
    """Select changed workspace packages and every workspace consumer."""
    members = set(metadata["workspace_members"])
    packages = {package["id"]: package for package in metadata["packages"]
                if package["id"] in members}
    ownership = {}
    for package_id, package in packages.items():
        manifest = pathlib.Path(package["manifest_path"])
        directory = pathlib.PurePosixPath(manifest.relative_to(root).parent.as_posix())
        if directory != pathlib.PurePosixPath("."):
            ownership[package_id] = (directory,)
        else:
            targets = tuple(
                pathlib.PurePosixPath(
                    pathlib.Path(target["src_path"]).relative_to(root).as_posix()
                )
                for target in package["targets"]
            )
            roots = tuple(pathlib.PurePosixPath(path.parts[0]) for path in targets
                          if path.parts and path.parts[0] in {"src", "tests", "benches"})
            ownership[package_id] = targets + roots

    direct = set()
    for raw_path in changed_paths:
        path = pathlib.PurePosixPath(raw_path)
        if path.is_absolute() or ".." in path.parts:
            raise ValueError(f"Git reported an invalid changed path: {raw_path!r}")
        owners = {package_id for package_id, roots in ownership.items()
                  if any(path == owned or owned in path.parents for owned in roots)}
        if len(owners) != 1:
            return None
        direct.update(owners)
    if not direct:
        return None

    consumers = {package_id: set() for package_id in packages}
    for node in metadata["resolve"]["nodes"]:
        if node["id"] not in packages:
            continue
        for dependency in node["dependencies"]:
            if dependency in packages:
                consumers[dependency].add(node["id"])
    selected = set(direct)
    pending = list(direct)
    while pending:
        dependency = pending.pop()
        for consumer in consumers[dependency]:
            if consumer not in selected:
                selected.add(consumer)
                pending.append(consumer)
    return tuple(sorted(packages[package_id]["name"] for package_id in selected))


def package_flags(packages):
    """Return Cargo package selectors for one pull-request closure."""
    return [argument for package in packages for argument in ("-p", package)]


REGISTRY_BOUNDARIES = {
    # Native entry logging and test span capture use the canonical tracing
    # crates; no Atlas provider supplies a tracing subscriber implementation.
    ("metis-host", "tracing-subscriber"),
    ("metis-core", "tracing"),
    ("metis-core", "tracing-subscriber"),
    ("metis-web", "tracing"),
    ("metis-web", "tracing-subscriber"),
    ("metis-cli", "tracing"),
    ("metis-cli", "tracing-subscriber"),
    ("metis-backend", "tracing"),
    # Development-only private WebView protocol tests capture scoped spans.
    ("metis-app", "tracing"),
    ("metis-app", "tracing-subscriber"),
    ("metis-backend", "tracing-subscriber"),
    # PNG admission uses the strict bounded host contract in ADR 0029.
    ("metis-ui-lang", "png"),
    # Independent Rust encoder manufactures small JPEG test inputs.
    ("metis-ui-lang", "jpeg-encoder"),
    # png tolerates a missing zlib trailer after producing all pixels; the
    # inflater separately requires stream end and the exact scanline extent.
    ("metis-ui-lang", "flate2"),
    ("metis-cli", "serde"),
    ("metis-cli", "serde_json"),
    ("metis-app", "serde"),
    ("metis-app", "serde_json"),
    ("metis-python", "pyo3"),
    # No Atlas provider exposes Unicode extended grapheme segmentation; this
    # pure-Rust dependency is the bounded browser text-policy boundary.
    ("metis-web", "unicode-segmentation"),
    # Development-only measurement instrument for the software rasterizer and
    # the whole-frame repaint; no Atlas provider supplies statistical benchmark
    # sampling, and the crate never enters a shipped artifact.
    ("metis-platform", "criterion"),
    ("metis-frontend", "criterion"),
    # The layout benches measure relayout and reprojection reuse; same
    # development-only instrument, never in a shipped artifact.
    ("metis-ui-lang", "criterion"),
    # The packaged WebView2 page speaks JSON to the host bridge, like the CLI
    # manifest and the application protocol above.
    ("metis-webview-page", "serde"),
    ("metis-webview-page", "serde_json"),
    # No Atlas provider supplies the wasm-bindgen browser ABI; the page uses
    # exact-pinned interop versions recorded in the audited lock, so a caret
    # range cannot drift the locked graph.
    ("metis-webview-page", "wasm-bindgen"),
    ("metis-webview-page", "wasm-bindgen-futures"),
    ("metis-webview-page", "js-sys"),
    ("metis-webview-page", "web-sys"),
}


def registry_boundary(package, dependency, source):
    """Determine whether a direct dependency uses its admitted registry boundary."""
    if dependency == "eunomia":
        return (source == "registry+https://github.com/rust-lang/crates.io-index"
                and package in {"metis-platform", "metis-ui-lang"})
    return source.startswith("registry+") and (package, dependency) in REGISTRY_BOUNDARIES


def package_version(lock: pathlib.Path, name: str) -> str:
    """Return the one locked package version required by an external ABI tool."""
    with lock.open("rb") as source:
        packages = [entry for entry in tomllib.load(source).get("package", ())
                    if entry.get("name") == name]
    if len(packages) != 1 or not isinstance(packages[0].get("version"), str):
        raise ValueError(f"Cargo lock must contain exactly one versioned package named {name}")
    return packages[0]["version"]


def application_targets(metadata):
    """Pin executable ownership independently of the application manifest."""
    members = [package for package in metadata["packages"] if package["id"] in metadata["workspace_members"]]
    binaries = sorted((package["name"], target["name"])
                      for package in members for target in package["targets"] if "bin" in target["kind"])
    if binaries != [("metis-app", "metis-app"), ("metis-cli", "metis"), ("metis-host", "metis-host")]:
        raise ValueError(f"Workspace must expose only the application, distribution and Apple host executables: {binaries}")
    for name in ("metis-backend", "metis-frontend"):
        libraries = [package for package in members if package["name"] == name]
        if len(libraries) != 1 or not any("lib" in target["kind"] for target in libraries[0]["targets"]):
            raise ValueError(f"Process role must remain a library: {name}")
    return binaries


def format_neutral_workspace(metadata):
    """Keep medical-format dependencies in RITK rather than the Metis host."""
    members = [package for package in metadata["packages"] if package["id"] in metadata["workspace_members"]]
    violations = []
    for package in members:
        if "dicom" in package["name"].casefold():
            violations.append(package["name"])
        violations.extend(
            f"{package['name']} -> {dependency['name']}"
            for dependency in package.get("dependencies", ())
            if "dicom" in dependency["name"].casefold()
        )
    if violations:
        joined = ", ".join(sorted(set(violations)))
        raise ValueError(f"Metis must remain format-neutral; DICOM belongs in RITK: {joined}")
