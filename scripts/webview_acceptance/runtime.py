"""Observe installed browser versions only from the application's actual descendants."""

import base64
import json
import subprocess
import time

from native_capture.windows.processes import process_tree

from .contract import MAX_UIA_BYTES


def version(process_id: int, deadline: float) -> dict:
    owned = sorted(process_tree(process_id))
    if not owned or len(owned) > 256:
        raise ValueError("actual WebView process tree exceeds its 256-process observation budget")
    identifiers = ",".join(map(str, owned))
    script = f"""
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$owned = @({identifiers})
$browsers = @(Get-Process -Name msedgewebview2 | Where-Object {{ $_.Id -in $owned }})
if ($browsers.Count -eq 0) {{ throw 'No actual owned WebView2 browser process is observable' }}
@($browsers | ForEach-Object {{
    [pscustomobject]@{{ pid = $_.Id; path = $_.MainModule.FileName;
        product_version = $_.MainModule.FileVersionInfo.ProductVersion }}
}}) | ConvertTo-Json -Depth 3 -Compress
"""
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        raise TimeoutError("actual runtime observation exceeds the fixture deadline")
    result = subprocess.run(
        ["powershell.exe", "-NoProfile", "-NonInteractive", "-EncodedCommand",
         base64.b64encode(script.encode("utf-16-le")).decode("ascii")],
        capture_output=True, timeout=remaining, check=True,
    )
    if len(result.stdout) > MAX_UIA_BYTES:
        raise ValueError("actual runtime observation exceeds its byte budget")
    observed = json.loads(result.stdout.decode("utf-8"))
    rows = observed if isinstance(observed, list) else [observed]
    if not rows or any(row.get("pid") not in owned or not row.get("path") or not row.get("product_version") for row in rows):
        raise ValueError("actual runtime metadata has an unowned or incomplete browser observation")
    versions = sorted({row["product_version"] for row in rows})
    if len(versions) != 1:
        raise ValueError(f"actual owned browser processes disagree on installed version: {versions}")
    return {"version": versions[0], "processes": rows}
