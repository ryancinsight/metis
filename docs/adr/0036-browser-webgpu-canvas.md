# ADR 0036: Explicit browser WebGPU canvas surface

Status: Accepted

Date: 2026-09-16

Driver: [METIS-GRAPHICS-001](../../backlog.md#METIS-GRAPHICS-001)

Upstream decision: [Moirai ADR 0061](../../../moirai/docs/adr/0061-browser-webgpu-canvas.md); recovery extension: [Moirai ADR 0063](../../../moirai/docs/adr/0063-browser-webgpu-recovery.md)

Revision 2026-09-18: Moirai PR #400 merged at
`7aa9d4e27fe8d12c0129accddccbf70bbc796e08` and adds `WebGpuCanvas::recreate`, which
acquires replacement browser GPU handles before swapping them into the
surface. `CanvasSurface::recreate` preserves the canvas and input listener
guards, clears the configured extent after success, and returns the provider's
typed setup error without falling back to raster presentation. At that revision,
real device-loss and recovered-pixel evidence remained unmeasured.

Revision 2026-09-19: the format-neutral `canvas_recovery` example and bounded
WebDriver runner exercise the public Metis surface on Edge 154.0.4258.24,
Windows, with an NVIDIA Blackwell adapter (`isFallbackAdapter = false`).
At device scale 1, all 1,024 initial screenshot pixels match opaque red/green.
Destroying the actual provider device resolves its `lost` promise with reason
`destroyed`. Against Moirai `8a8daa60cca5484822c772bc6b574acaa8f133ad`, the
subsequent `present` incorrectly reports success: the browser does not throw
a synchronous upload exception. This measured failure belongs to Moirai's
device-loss observation, not Metis's renderer dispatch. Explicit recreation
already replaces the device and redraws all 1,024 pixels as opaque blue/white;
the canvas identity and seven retained input guards survive, and stopping
drops the retained guard count to zero.

Moirai correction `98579514ae96ca49475a7a0f14d47b65e6872a41` observes each
device's loss promise independently and rejects presentation after observation.
The repeated browser gate reports `Other: WebGPU device is lost` before any
post-loss upload, then passes recreation with the same exact pixel results.
The trace records configuration and upload device sequences `[1, 2]`, no
uncaptured GPU errors, and successful WebDriver session closure. Loss becomes
visible after the browser delivers the promise; this is not synchronous
detection of a driver fault. Acquisition-failure atomicity and observer
cancellation were source-reviewed, not browser fault-injected.

## Context

Metis owns the format-neutral browser canvas contract and RITK owns DICOM
decoding and clinical presentation. The existing `CanvasSurface` presents
borrowed RGBA8 frames through Moirai's two-dimensional HTML5 canvas provider.
RITK's browser viewer needs an explicit route to a device-backed browser
surface so GPU presentation can be measured without moving rendering policy or
format semantics into Metis. WebGPU is an asynchronous browser capability and
must not be inferred from the presence of a canvas element.

## Decision

`CanvasSurface` stores one private renderer enum with the existing raster
provider and Moirai's `WebGpuCanvas`. Existing synchronous constructors retain
the raster path. Four asynchronous constructors (`from_*_gpu` and
`from_*_gpu_with_input`) opt into WebGPU and return the provider's typed
unsupported or setup error when an adapter, device, context or listener cannot
be acquired. They never fall back to the raster provider.

Both renderers consume the same borrowed `CanvasFrame` contract, validate the
same dimensions and byte count, expose the same canvas identifier and retain
the same bounded pointer, wheel and keyboard listener guards. Renderer choice
does not alter event trust, queue bounds, teardown or ownership of source
pixels. Metis does not inspect the consumer's data format; RITK remains the
owner of DICOM scanning, decoding, geometry, slice state and image oracles.

The provider follows the [WebGPU canvas context specification](https://www.w3.org/TR/webgpu/#canvas-context)
and [GPUQueue.copyExternalImageToTexture contract](https://developer.mozilla.org/en-US/docs/Web/API/GPUQueue/copyExternalImageToTexture).

## Alternatives rejected

1. Selecting WebGPU automatically would make capability and failure behavior
   implicit and would silently change presentation evidence across browsers.
2. Falling back to the raster provider after a WebGPU failure would hide a
   device or permission fault and invalidate GPU measurements.
3. Adding a second RITK-only Metis adapter would duplicate the host contract;
   the renderer enum keeps one presentation and input surface.

## Failure modes and limits

Missing WebGPU, adapter denial, device setup failure, context loss and browser
upload rejection must remain visible as typed errors. The surface retains no
frame bytes after submission. The measured opaque two-color recovery case
does not establish alpha blending, vector parity, performance, device memory
use, spontaneous driver-reset recovery, or behavior in other browsers.

## Verification

The locked Metis native nextest suite and strict WASM checks cover the raster
contract and compile the WebGPU renderer dispatch and recovery example.
`python scripts/browser_gpu_recovery.py` runs the device-loss regression with
10-second asynchronous stage deadlines, at most four recorded devices and 64
records per GPU event stream, and a 512 KiB JSON limit. It replaces the latest
trace and two PNGs under `output/browser/gpu-recovery`. The trace binds source, lockfile and WASM
digests to the run and reports dirty-tree state. See [V06](../VERIFICATION.md#V06)
for measured outcomes and limits. RITK's clinical images remain separate
consumer-owned evidence.
