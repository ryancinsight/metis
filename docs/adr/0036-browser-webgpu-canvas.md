# ADR 0036: Explicit browser WebGPU canvas surface

Status: Accepted

Date: 2026-09-16

Driver: [METIS-GRAPHICS-001](../backlog.md#METIS-GRAPHICS-001)

Upstream decision: [Moirai ADR 0061](../../moirai/docs/adr/0061-browser-webgpu-canvas.md); recovery extension: [Moirai ADR 0063](../../moirai/docs/adr/0063-browser-webgpu-recovery.md)

Revision 2026-09-18: Moirai PR #400 merged at
`7aa9d4e27fe8d12c0129accddccbf70bbc796e08` and adds `WebGpuCanvas::recreate`, which
acquires replacement browser GPU handles before swapping them into the
surface. `CanvasSurface::recreate` preserves the canvas and input listener
guards, clears the configured extent after success, and returns the provider's
typed setup error without falling back to raster presentation. Real device-loss
and recovered-pixel evidence remain consumer-owned browser requirements.

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
upload rejection remain visible as typed errors. The surface retains no frame
bytes after submission. This increment does not claim a GPU device exists on a
particular browser, compositor latency, or visual equivalence; those require a
real browser run and are recorded by the RITK consumer evidence.

## Verification

The locked Metis native nextest suite and strict WASM library checks cover the
unchanged raster contract and compile the WebGPU renderer dispatch. RITK's
consumer adds an opt-in GPU entry point and must supply browser capability and
pixel evidence separately from the existing 2D real-study galleries.
