"""Exercise real executable, MSI, installation, and user-file preservation workflows.

Only output/distribution/latest is retained. Native installation is opt-in and
uses an isolated INSTALLDIR, the current user's registry and Start Menu.
"""
import argparse
import ctypes
import hashlib
import json
import math
import os
import pathlib
import re
import shutil
import stat
import subprocess
import time
from fractions import Fraction

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "output" / "distribution"
MARKER = "metis-distribution-workflow-1\n"
TOTAL_SECONDS = 720
LOG_LIMIT = 16 * 1024 * 1024
TREE_LIMIT = 4 * 1024 * 1024 * 1024


def unredirected(path):
    """Read-only inputs may share storage, but cannot redirect path traversal."""
    for part in (path, *path.parents):
        if part.is_symlink():
            raise ValueError(f"Linked workflow path: {part}")
        if part.exists():
            metadata = part.lstat()
            if getattr(metadata, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT:
                raise ValueError(f"Reparse workflow path: {part}")
    return path


def unlinked(path):
    """Owned outputs cannot alias another file through a hardlink."""
    unredirected(path)
    if path.is_file() and path.stat().st_nlink != 1:
        raise ValueError(f"Multiply linked workflow file: {path}")
    return path


def tree_files(directory):
    """Enumerate bounded regular files without traversing links."""
    files = []
    total = 0
    unlinked(directory)
    for base, directories, names in os.walk(directory, followlinks=False):
        for name in (*directories, *names):
            path = unlinked(pathlib.Path(base) / name)
            if path.is_file():
                total += path.stat().st_size
                files.append(path)
                if len(files) > 16384 or total > TREE_LIMIT:
                    raise ValueError("Workflow artifacts exceed 16384 files or 4 GiB")
            elif not path.is_dir():
                raise ValueError(f"Non-regular workflow artifact: {path}")
    return files


def product_state(product):
    """Read Windows Installer registration for the current user."""
    if os.name != "nt":
        raise ValueError("MSI registration checks require Windows")
    library = ctypes.WinDLL("msi", use_last_error=True)
    query = library.MsiQueryProductStateW
    query.argtypes = [ctypes.c_wchar_p]
    query.restype = ctypes.c_int
    return query(product)


def prepare(output):
    """Evict only the previous marked run at the exact committed output root."""
    if pathlib.Path(os.path.abspath(output)) != OUTPUT:
        raise ValueError(f"Workflow output must be {OUTPUT}")
    unlinked(OUTPUT)
    OUTPUT.mkdir(parents=True, exist_ok=True)
    latest = OUTPUT / "latest"
    if latest.exists():
        tree_files(latest)
        marker = latest / ".workflow"
        if not marker.is_file() or marker.read_text(encoding="utf-8") != MARKER:
            raise ValueError("Refusing to erase an unowned distribution directory")
        inventory = latest / "package" / "inventory.json"
        if inventory.is_file():
            product = (read_json(inventory).get("installer") or {}).get("product_code")
            if product and product_state(product) != -1:
                raise ValueError(f"Previous test installation is still registered: {product}; uninstall it before rerunning")
        # The absolute target and every descendant have been checked above;
        # no other output directory or user installation is a deletion target.
        shutil.rmtree(latest)
    latest.mkdir()
    (latest / ".workflow").write_text(MARKER, encoding="utf-8", newline="\n")
    return latest


def read_json(path):
    unlinked(path)
    if path.stat().st_size > LOG_LIMIT:
        raise ValueError("Workflow JSON exceeds its 16 MiB limit")
    return json.loads(path.read_text(encoding="utf-8"))


def digest(path):
    unlinked(path)
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def destination(root, name):
    """Inventory paths cannot escape their application directory."""
    if not isinstance(name, str) or not name or "\\" in name or ":" in name:
        raise ValueError("Invalid inventory destination")
    if any(part in ("", ".", "..") for part in name.split("/")) or name.startswith("/"):
        raise ValueError("Invalid inventory destination")
    return unlinked(root.joinpath(*name.split("/")))


def verify_payload(directory, inventory, *, extras=()):
    records = inventory["files"]
    if not 1 <= len(records) <= 4096:
        raise ValueError("Invalid payload inventory size")
    expected = set()
    for record in records:
        name = record["destination"]
        if name in expected:
            raise ValueError("Duplicate inventory destination")
        expected.add(name)
        path = destination(directory, name)
        if not path.is_file() or path.stat().st_size != record["bytes"] or digest(path) != record["sha256"]:
            raise ValueError(f"Payload does not match its inventory: {name}")
    actual = {path.relative_to(directory).as_posix() for path in tree_files(directory)}
    if actual != expected | set(extras):
        raise ValueError(f"Unexpected payload file set: {sorted(actual ^ (expected | set(extras)))}")
    return sorted(expected)


def verify_application(directory, inventory):
    """The shipped demonstration uses one image for both isolated process roles."""
    application = inventory["application"]
    suffix = ".exe" if os.name == "nt" else ""
    entry = "metis-app" + suffix
    if (application["entry"] != "metis-app"
            or application["binaries"] != [{"package": "metis-app", "bin": "metis-app"}]
            or inventory["entry"] != entry):
        raise ValueError("Demonstration requires exactly one metis-app executable and entry")
    payload = verify_payload(directory, inventory)
    expected = {entry} | {resource["destination"] for resource in application["resources"]}
    if set(payload) != expected:
        raise ValueError("Package contains a different application inventory")
    # Resource declarations cannot disguise a second Windows executable.
    executables = [name for name in payload if pathlib.PurePosixPath(name).suffix.casefold() == ".exe"
                   or name == entry]
    if executables != [entry]:
        raise ValueError("Demonstration payload must contain exactly one application executable")
    return payload


def verify_registration(values, inventory, installed):
    """Pin component ownership and the persisted uninstall destination."""
    expected = {f"F{index}": inventory["application"]["version"]
                for index in range(len(inventory["files"]))}
    if set(values) != set(expected) | {"InstallLocation"}:
        raise ValueError("Installed component registry names differ from inventory")
    if any(values[name] != value for name, value in expected.items()):
        raise ValueError("Installed component registry versions differ from inventory")
    if pathlib.Path(values["InstallLocation"]) != installed:
        raise ValueError("Installer did not persist its actual installation directory")


def calculation(output, arguments):
    """Compare the two-process result to exact rational unit conversion."""
    match = re.search(r"frontend_pid=(\d+) rate_ml_hr=([^\s]+) drug_rate_mg_hr=([^\s]+) audit_sequence=(\d+)", output)
    backend = re.search(r"backend_pid=(\d+)", output)
    audit = re.search(r"Metis session completed; (\d+) audit records verified", output)
    if not match or not backend or not audit or match[1] == backend[1]:
        raise ValueError("Application did not complete a distinct frontend/backend session")
    weight, concentration, dose = map(Fraction, arguments)
    expected = (dose * weight * 60 / (concentration * 1000), dose * weight * 60 / 1000)
    actual = tuple(map(float, (match[2], match[3])))
    for value, reference in zip(actual, expected, strict=True):
        # Three parsed inputs and five rounded arithmetic operations give an
        # eight-epsilon relative bound; these positive inputs have no cancellation.
        bound = abs(reference) * Fraction(8, 2 ** 52)
        if not math.isfinite(value) or abs(Fraction(value) - reference) > bound:
            raise ValueError(f"Calculation differs from rational oracle: {value} versus {reference}")
    if int(audit[1]) < 1 or int(match[4]) > int(audit[1]):
        raise ValueError("Calculation audit sequence is outside the verified ledger")
    return {"arguments": list(arguments), "rate_ml_hr": actual[0], "drug_rate_mg_hr": actual[1],
            "frontend_pid": int(match[1]), "backend_pid": int(backend[1]), "audit_records": int(audit[1])}


class Workflow:
    def __init__(self, directory):
        self.directory = directory
        self.deadline = time.monotonic() + TOTAL_SECONDS
        self.report = {"schema": 1, "status": "running", "commands": [], "calculations": []}
        self.save()

    def save(self):
        unlinked(self.directory / "workflow.json").write_text(
            json.dumps(self.report, indent=2) + "\n", encoding="utf-8", newline="\n")

    def run(self, name, arguments, limit=60):
        # Reserve one full command budget for uninstall after any verification failure.
        remaining = self.deadline - time.monotonic() - (0 if name == "uninstall" else 60)
        if remaining <= 0:
            raise TimeoutError("Distribution workflow exceeded its 720 s budget")
        path = unlinked(self.directory / f"{name}.log")
        entry = {"name": name, "arguments": [str(value) for value in arguments]}
        self.report["commands"].append(entry)
        self.save()
        with path.open("xb") as output:
            result = subprocess.run(entry["arguments"], stdout=output, stderr=subprocess.STDOUT,
                                    stdin=subprocess.DEVNULL, timeout=min(limit, remaining), check=False)
        entry["exit_code"] = result.returncode
        self.save()
        if path.stat().st_size > LOG_LIMIT:
            raise ValueError(f"Command log exceeded 16 MiB: {name}")
        text = path.read_text(encoding="utf-8", errors="replace")
        if result.returncode != 0:
            raise ValueError(f"{name} exited {result.returncode}; see {path}")
        return text

    def applications(self, directory, inventory, label):
        first = inventory["application"]["arguments"]
        if first != ["60", "2", "0.2"]:
            raise ValueError("Demonstration manifest changed; update the declared workflow scenarios")
        for index, arguments in enumerate((first, ["80", "4", "0.5"])):
            result = self.run(f"{label}-{index}", [destination(directory, inventory["entry"]), *arguments])
            self.report["calculations"].append({"location": label, **calculation(result, arguments)})
        self.save()

    def install(self, package, inventory):
        if os.name != "nt":
            raise ValueError("Installation workflow requires Windows")
        import winreg
        installer = inventory["installer"]
        product = installer["product_code"]
        if product_state(product) != -1:
            raise ValueError("Test product is already registered")
        folder = self.run("programs-directory", ["powershell.exe", "-NoProfile", "-NonInteractive", "-Command",
                          "[Console]::OutputEncoding = [Text.UTF8Encoding]::new(); [Environment]::GetFolderPath('Programs')"]).strip()
        shortcut = unlinked(pathlib.Path(folder) / inventory["application"]["id"] / f'{inventory["application"]["name"]}.lnk')
        key = f'Software\\Metis\\Applications\\{inventory["application"]["id"]}'
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key):
                raise ValueError("An application with this identity already owns the test registry key")
        except FileNotFoundError:
            pass
        if shortcut.parent.exists():
            raise ValueError("Test identity Start Menu directory must be absent")
        installed = unlinked(self.directory / "installed")
        if installed.exists():
            raise ValueError("Test installation directory must be absent")
        msiexec = pathlib.Path(os.environ["SystemRoot"]) / "System32" / "msiexec.exe"
        sentinel = installed / "user-created.txt"
        sentinel_bytes = b"User-created content must survive uninstall.\n"
        try:
            self.run("install", [msiexec, "/i", destination(package, installer["file"]), "/qn", "/norestart",
                                f"INSTALLDIR={installed}", "/L*v", self.directory / "install-msi.log"])
            if product_state(product) != 5:
                raise ValueError("Windows Installer did not register the installed product")
            verify_application(installed, inventory)
            if not shortcut.is_file() or shortcut.stat().st_size == 0:
                raise ValueError("Installer did not create its Start Menu shortcut")
            quoted_shortcut = str(shortcut).replace("'", "''")
            shortcut_script = (
                "[Console]::OutputEncoding = [Text.UTF8Encoding]::new(); "
                "$shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut('" + quoted_shortcut + "'); "
                "@{target=$shortcut.TargetPath; arguments=$shortcut.Arguments; working_directory=$shortcut.WorkingDirectory; "
                "icon_location=$shortcut.IconLocation} | ConvertTo-Json -Compress")
            shortcut_fields = json.loads(self.run("shortcut", ["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", shortcut_script]))
            icon_location = shortcut_fields.get("icon_location")
            if (pathlib.Path(shortcut_fields["target"]) != destination(installed, inventory["entry"])
                    or shortcut_fields["arguments"] != '"60" "2" "0.2"'
                    or pathlib.Path(shortcut_fields["working_directory"]) != installed
                    or not isinstance(icon_location, str)
                    or not icon_location.casefold().endswith("\\metisicon,0")):
                raise ValueError("Start Menu shortcut fields do not match the declared application")
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key) as registry:
                values = {winreg.EnumValue(registry, index)[0]: winreg.EnumValue(registry, index)[1]
                          for index in range(winreg.QueryInfoKey(registry)[1])}
            verify_registration(values, inventory, installed)
            self.applications(installed, inventory, "installed")
            sentinel.write_bytes(sentinel_bytes)
            self.report["installation"] = {"product_code": product, "registered": True,
                                            "shortcut": str(shortcut), "shortcut_fields": shortcut_fields, "payload_verified": True}
            self.save()
        finally:
            # The generated ProductCode is unique and preflight confirmed absence.
            # Supply no INSTALLDIR: maintenance must recover its saved location.
            # Uninstall only that product, including after a failed verification.
            if product_state(product) != -1:
                self.run("uninstall", [msiexec, "/x", product, "/qn", "/norestart",
                                      "/L*v", self.directory / "uninstall-msi.log"])
        if product_state(product) != -1 or shortcut.parent.exists():
            raise ValueError("Uninstall retained product registration or its empty identity Start Menu directory")
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key):
                raise ValueError("Uninstall retained the owned component registry key")
        except FileNotFoundError:
            pass
        if not sentinel.is_file() or sentinel.read_bytes() != sentinel_bytes:
            raise ValueError("Uninstall removed or modified the user-created file")
        remaining = {path.relative_to(installed).as_posix() for path in tree_files(installed)}
        if remaining != {sentinel.name}:
            raise ValueError(f"Uninstall retained owned payload files: {sorted(remaining)}")
        self.report["installation"].update(uninstalled=True, user_file_preserved=True, start_menu_directory_removed=True)
        self.save()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tool", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--install", action="store_true", help="Install and uninstall the generated per-user MSI")
    args = parser.parse_args()
    if not args.tool.is_absolute() or not unredirected(args.tool).is_file():
        parser.error("--tool must name an absolute built Metis executable")
    directory = prepare(args.output)
    workflow = Workflow(directory)
    try:
        package = directory / "package"
        workflow.run("package", [args.tool, "package", ROOT / "metis.json", package], 300)
        inventory = read_json(package / "inventory.json")
        source = read_json(ROOT / "metis.json")
        if inventory["schema"] != 1 or inventory["application"] != source:
            raise ValueError("Package changed the declared application manifest")
        verify_application(package / "app", inventory)
        for resource in source["resources"]:
            if digest(destination(ROOT, resource["source"])) != digest(destination(package / "app", resource["destination"])):
                raise ValueError("Packaged resource differs from its declared source")
        installer = inventory["installer"]
        if not re.fullmatch(r"\{[0-9A-F]{8}(?:-[0-9A-F]{4}){3}-[0-9A-F]{12}\}", installer["product_code"]):
            raise ValueError("Invalid installer ProductCode")
        if digest(destination(package, installer["file"])) != installer["sha256"]:
            raise ValueError("Installer hash differs from its inventory")
        workflow.report["inventory"] = inventory
        workflow.applications(package / "app", inventory, "portable")
        if args.install:
            workflow.install(package, inventory)
        else:
            workflow.report["installation"] = {"status": "not requested"}
        tree_files(directory)
        workflow.report["status"] = "passed"
    except Exception as error:
        workflow.report.update(status="failed", error=str(error))
        raise
    finally:
        workflow.save()
    print(directory / "workflow.json")


if __name__ == "__main__":
    main()
