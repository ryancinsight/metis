"""Verification source identity and inherited build configuration."""

import pathlib
import tomllib


def resolution(lock):
    """Preserve every byte of the standalone lock, rejecting overlay residue."""
    content = lock.read_bytes()
    if "unused" in tomllib.loads(content.decode("utf-8")).get("patch", {}):
        raise SystemExit(
            "Cargo.lock contains Atlas overlay residue; regenerate standalone before verification"
        )
    return content


def inherited_configuration(root, environment):
    """Retain ancestor build budgets without importing their source patches."""
    target = None
    profiles = {}
    configs = []

    def leaves(table, prefix=()):
        for key, value in table.items():
            path = (*prefix, key)
            if isinstance(value, dict):
                yield from leaves(value, path)
            else:
                yield path, value

    for directory in reversed((root, *root.parents)):
        candidates = [directory / ".cargo" / name for name in ("config", "config.toml")]
        config = next((path for path in candidates if path.is_file()), None)
        if config is None:
            continue
        configs.append(config)
        content = tomllib.loads(config.read_text(encoding="utf-8"))
        configured = content.get("build", {}).get("target-dir")
        if configured is not None:
            target = (directory / configured).resolve()
        profiles.update(leaves(content.get("profile", {}), ("profile",)))
    if environment.get("CARGO_TARGET_DIR"):
        target = pathlib.Path(environment["CARGO_TARGET_DIR"]).resolve()
    return target, profiles, configs


def state(root, metadata, configs):
    """Hash every source and configuration input consumed by the gate."""
    from visual import source_digest

    inputs = {root / "rust-toolchain.toml", root / "metis.json", root / "deny.toml", *configs}
    inputs.update(path for path in (root / "examples" / "browser").rglob("*") if path.is_file())
    workflow_root = root / ".github" / "workflows"
    if workflow_root.is_dir():
        inputs.update(path for path in workflow_root.iterdir()
                      if path.is_file() and path.suffix in {".yml", ".yaml"})
    inputs.update((root / "docs").rglob("*.md"))
    inputs.update(path for path in (root / "scripts").rglob("*")
                  if path.is_file() and "__pycache__" not in path.parts)
    for package in metadata["packages"]:
        if package["source"] is None:
            manifest = pathlib.Path(package["manifest_path"])
            inputs.add(manifest)
            directory = manifest.parent
            inputs.update(directory.glob("*.rs"))
            inputs.update(directory.glob("*.md"))
            # `fonts` holds typefaces the renderer embeds, a rendering input.
            for folder in ("src", "tests", "examples", "scripts", "python", ".config", "fonts"):
                inputs.update(path for path in (directory / folder).rglob("*")
                              if path.is_file() and "__pycache__" not in path.parts)
            if package["name"] == "metis-python":
                inputs.add(directory / "pyproject.toml")
    return {str(path.resolve()): source_digest(path) for path in sorted(inputs)}
