"""Workspace executable ownership and direct dependency contracts."""

REGISTRY_BOUNDARIES = {
    # Native entry logging and test span capture use the canonical tracing
    # crates; no Atlas provider supplies a tracing subscriber implementation.
    ("metis-host", "tracing-subscriber"),
    ("metis-cli", "tracing"),
    ("metis-cli", "tracing-subscriber"),
    ("metis-backend", "tracing"),
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
}


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

