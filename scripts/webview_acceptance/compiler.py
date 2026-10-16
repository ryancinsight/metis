"""Select actual compiler artifacts; never infer executable names from folders."""

import json
from pathlib import Path


def executable(output: str, package: str, target: str, *, tests: bool = False) -> Path:
    selected = set()
    for line in output.splitlines():
        if not line.startswith("{"):
            continue
        record = json.loads(line)
        owner = record.get("package_id", "")
        if (record.get("reason") == "compiler-artifact"
                and record.get("target", {}).get("name") == target
                and bool(record.get("profile", {}).get("test")) == tests
                and record.get("executable")
                and (owner.endswith(f"#{package}") or f"#{package}@" in owner
                     or f"/{package}#" in owner)):
            selected.add(Path(record["executable"]))
    if len(selected) != 1:
        raise ValueError(f"compiler must produce exactly one {package}/{target} executable: {selected}")
    path = selected.pop().resolve(strict=True)
    if not path.is_file():
        raise ValueError("compiler executable does not name a real file")
    return path
