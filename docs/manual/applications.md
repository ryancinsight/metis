# Application gallery

These are captures of the production Metis form and its real backend responses,
rendered into an 800×600 software framebuffer through Iris. The runnable
[presentation example](../../examples/presentation.rs) uses bounded IPC and a
Moirai worker, asserts values and audit outcomes, and checks text geometry before
capturing each state. That software example does not create native controls or
execute in a browser; the separate [browser workbench](browser.md) exercises the
HTML5/CSS host.
The separate [process demonstration](getting-started.md) tests actual child processes.

## Real DICOM application evidence

The browser DICOM gallery has a slice slider beneath each anatomical view.
Drag a slider to browse that plane, or focus it and use the arrow keys;
Home and End select its first and last slices. Each counter shows the current
slice and the total available in that plane. Wheel navigation and cine playback
update the same controls. RITK owns slice selection and reformatting; the gallery
passes the selected axis and index to its Rust viewer.

The gallery starts with a real saved-study run so the first image is application
output rather than generated artwork. RITK opened the public MRI-DIR head CT
series, decoded the 409 DICOM image instances in the 410-entry study directory,
and supplied axial, coronal and sagittal planes to the format-neutral Métis
framebuffer. The packaged Windows executable exited
0 and the portable and per-user MSI captures matched byte-for-byte.

![Actual saved CT study rendered through the packaged Métis host](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-installer-ct.png?raw=true)

The [package provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-installer.json)
binds the source manifest, executable and MSI digests, capture dimensions,
non-black panel pixels and install/uninstall results. The companion
[browser component-state record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-components.json)
records a mounted generation, Rust-owned listener handles, accepted file bytes,
theme, released pointer capture, idle text composition and clean teardown. The
[saved local clinical-study command](applications.md#open-a-saved-local-clinical-study) uses
the same RITK-owned launcher for private DICOM; its pixels and identifiers stay
on the local machine.

On Windows, the same RITK launcher can omit the positional path and use the
bounded Moirai folder picker through Métis. The selected folder is scanned and
decoded by RITK, while cancellation and an unreadable study leave the existing
frame unchanged. The [RITK DICOM manual](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md)
contains the actual picker image, capture provenance and study-reopen workflow.

![Actual MRI study after native folder selection through Métis](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-picker-mri-window.jpg?raw=true)

The image is the RITK consumer capture from PR [#392](https://github.com/ryancinsight/ritk/pull/392),
merged at `3f46a08bf`. It shows the three decoded MRI planes after the user
selected the `DICOM` folder in the native dialog. Metis contributes the bounded
folder-selection result and window surface; RITK owns scanning, decoding and
clinical presentation. The [RITK MRI provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-mri.json)
binds the executable, input study and framebuffer hashes.

The merged Windows default-shell rerun is tracked separately in RITK's
[revision-bound provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-default-shell-ct.json).
It binds the current three-plane capture and repeated process-tree samples to
the exact RITK, Metis and Moirai revisions; the public image above is the
reviewed PNG emitted by that run.

## Starter theme and mark

The browser workbench ships a local starter vector mark for the application
header and favicon:

![Starter Métis mark](../../examples/browser/assets/metis-mark.svg)

The SVG is a local, scriptless vector asset with a fixed viewport and literal
path colors. It is a replaceable project asset, not a remote stock download.
Applications can keep the same `Theme` selector and replace the semantic CSS
variables and this SVG in their own browser asset directory. The browser build
also carries a PNG alternate for user agents that do not select SVG sources.

The package demonstration derives `metis-mark.ico` from the same local artwork
for the Windows Start Menu shortcut. Its seven embedded PNG resolutions are
validated by `metis-cli` before the MSI is written; the portable package keeps
the SVG, PNG and ICO under `assets/` as well. The CLI applies the bounded SVG
contract before copying the vector resource, rejecting XML expansion, external
references, unknown attributes, malformed geometry and oversized viewports.

## Deterministic synthetic DICOM baseline

RITK's [synthetic DICOM workflow](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md)
includes a capture of the running egui/eframe viewer alongside exact
software slice images. Selected-study workflows landed in
[RITK PR 236](https://github.com/ryancinsight/ritk/pull/236); physical display
proportions and the original viewer capture follow in
[RITK PR 237](https://github.com/ryancinsight/ritk/pull/237) at `8152f483`.
Its three-instance study has known decoded values,
anisotropic spacing and physical coordinates; no patient data is required.
The native workflow also requires a missing study to fail without producing a
successful-load screenshot.

The baseline exercises explicit series selection, primary and secondary loads,
authoritative DICOMDIR membership, failed replacement and session restore.
Physical image proportions now follow voxel spacing across layouts and texture
rotations, with paint and hit testing sharing a validated screen rectangle.
Patient-coordinate fusion and transformed measurement semantics remain required;
correct image proportions alone do not establish either property.
The manual explains the current input limits and how to reproduce the pixel
checks and native captures. These results establish the original viewer
baseline for [V09](../VERIFICATION.md#V09). Metis provides a bounded browser
named-byte batch handoff, while RITK owns the scanner, decoder, volume geometry
and medical display semantics. The merged
[RITK PR 269](https://github.com/ryancinsight/ritk/pull/269) adds the
`ritk-snap --metis-native` three-plane workflow: RITK opens and decodes the
study, selects the axial, coronal and sagittal planes, and produces one
spacing-aware `PresentationFrame`; Métis owns the native window, event pump and
framebuffer. Metis has no DICOM decoder or volume model. The follow-up
[RITK PR 271](https://github.com/ryancinsight/ritk/pull/271) keeps browser file
handles and bounded bytes in Métis while RITK performs the same classification
and decoding after the handoff.

The first Windows handoff opens the synthetic study, renders the three real
RITK planes, routes wheel navigation by panel and rejects a missing study. The
deterministic content capture is
[`dicom-metis-native.png`](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-native.png)
at 1280×800; it is a framebuffer oracle rather than an operating-system window
golden. Full migration acceptance still requires browser runtime capture,
multiframe/color presentation, matched memory measurements, complete
operating-system application-window capture and packaging evidence.

The native handoff has also been exercised with the acquired MRI-DIR CT series,
not only the generated Part 10 study. RITK's user manual records the command,
the public CC BY 4.0 source, the reviewed 1280×800 output and its SHA-256 in
[the actual DICOM capture section](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md).
That image is RITK application output transferred through the format-neutral
Métis framebuffer; Metis still owns no DICOM parser, metadata, geometry or
clinical display policy. Private clinical studies remain local and are never
committed to either repository.

![Actual MRI-DIR CT study rendered through the Métis native surface](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-ct.png?raw=true)

RITK also provides an explicit application-content capture for visual review.
The `--capture-application` option draws bounded plane, slice, frame-dimension
and window/level labels into the same Métis framebuffer after the real CT
planes are composed. The reviewed image is [the application-content capture](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-ct-application.png?raw=true), with its [provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-ct-application.json).
Operating-system decorations remain outside the capture; RITK retains the
DICOM and clinical display responsibilities.

![Actual MRI-DIR CT study with the RITK application overlay through Métis](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-ct-application.png?raw=true)

The current native host also presents an explicit CPU maximum-intensity
projection (MIP) quadrant for the same saved public CT series. RITK decodes
the 409-file study and computes the projection before handing four
format-neutral panels to Métis; the `--metis-native-layout orthogonal-with-mip`
option selects this layout. The reviewed capture is [the real native MIP
application output](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-ct-mip.png?raw=true),
and its file bounds, repeat digest and panel pixel counts are in the [MIP
provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-ct-mip.json).
This is public MRI-DIR phantom data, not a generated illustration or private
patient study.

![Actual MRI-DIR CT study with the RITK native MIP panel through Métis](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-ct-mip.png?raw=true)

The RITK eframe integration also exercises the asynchronous GPU projection
with a fitting volume from the same public series. RITK's GPU path waits for
the matching wgpu readback and invalidates stale frames when display
parameters change; the existing CPU path remains responsible for volumes that
exceed device limits. The reviewed application capture includes the three
orthogonal planes and a `3D MIP · GPU` label:

![Actual MRI-DIR CT study with GPU MIP in the eframe application](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-eframe-real-gpu-ct.png?raw=true)

The source-file list, repeat-run digest and graphics-backend selection are in
the [RITK GPU capture provenance](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-eframe-real-gpu-ct.json).
This is public MRI-DIR phantom data. The GPU projection and DICOM decisions
remain in RITK; Métis owns the format-neutral host and framebuffer boundary.

The same 409-file public CT was also run through the current eframe application
with the bounded lifecycle sampler. The run exited 0 three times and produced
the [real 1600×1000 viewer capture](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-eframe-real-ct.png?raw=true)
(the current local capture digest is
`f4b30c71bd57f54227f5eeec524cd72e93f56f68e834fed909b907f9ecace048`). Its
mean process-tree peak private bytes were 2,494,962,346.7 ± 5,291,688.9 bytes
and its mean lifecycle duration was 8,752.3 ± 2,197.2 ms; the complete
[resource provenance record](images/dicom-eframe-real-ct-resource.json) stores
the revisions, executable digest, input bounds and sample statistics. The
eframe baseline and the current native Métis MIP capture use the same public
series, but their presentation contracts differ: eframe labels its fourth
viewport `3d_mip`, while the Métis run uses RITK's `axial_mip` policy. Their
surface dimensions and process boundaries also differ, so the measurements
remain lifecycle evidence and do not establish a framework memory or latency
ranking. The Métis run is recorded in [its resource provenance](images/dicom-metis-real-ct-mip-resource.json).

The same public 94-file MRI-DIR T2 study was also opened through the real
eframe compatibility shell. The reviewed [1600×1000 MRI capture](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-eframe-real-mri.png?raw=true)
shows the series browser, three orthogonal planes and the `3D MIP · GPU`
projection. Its [resource provenance](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-eframe-real-mri-resource.json)
binds the executable and image digests to three equal lifecycle captures. This
is a baseline for the same public input as the Métis MRI row, but the surfaces
and process boundaries remain different.

![Actual MRI-DIR T2 study rendered in the eframe compatibility shell](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-eframe-real-mri.png?raw=true)

The shell-free eframe fixture now accepts a validated `--viewport-size`
logical-point contract. With `--viewport-size 1024x640`, the controlled 125%
Windows host produces the same 1280×800 physical extent as the Métis native
surface. The `orthogonal-surface` presentation keeps only the axial, coronal
and sagittal planes and reuses RITK's spacing-aware `ImagePlacement` path, so
its semantic surface set matches the Métis MRI record while DICOM loading and
geometry stay in RITK. The reviewed [1280×800 source capture](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-eframe-orthogonal-surface.png?raw=true)
contains real MRI anatomy and no series browser or MIP:

![Actual MRI-DIR T2 planes rendered in the matched eframe orthogonal surface (640×400 tracked figure from a 1280×800 source capture)](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-eframe-orthogonal-surface.png?raw=true)

Three lifecycle runs exited 0 with the same digest. The [matched provenance
record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-eframe-orthogonal-surface-resource.json)
binds RITK PR #487 merge `102909c8f67bca1128eba1438ec280ee2651656e`, the locked
Metis and Moirai revisions, the requested 1024×640 logical size, observed
1280×800 physical size, executable and PNG hashes, and the semantic keys `axial`,
`coronal`, `sagittal`. The semantic and host-extent prerequisites for V12 now
match; the tracked manual figure is 640×400 while provenance retains the raw
1280×800 source digest. The eframe process boundary still differs from Métis,
and GPUI/Tauri fixtures remain open, so no framework ranking is derived.

The current standalone-lock replay (2026-09-18) is bound to RITK source
`c93a4f67ef0bedc3eed8ea405e3917a8c0a62c59`, Metis
`860ffbf52d12a70c80dd2f150d2aff3a5630d57e` and Moirai
`5075d4c70ba4f840d4c5a47b67c5d564405badf5`. The standalone lock digest is
`f4a448b5bb3e76c1f966908c7b59ebc90b1102f26d668b5be72e64c9b611bbd2`, and the
executable digest is
`8a7ff0d40c119e19150f9ed9b643868eebff6d2865b6d3130e4a1014a3b7f995`. RITK
opened the saved 94-file MRI-DIR study (49,807,236 bytes) and produced the actual 1280 × 800
three-plane frame below; it contains 411,589 non-black pixels and repeats the
capture digest `259dd79103482756c4e688621bebafc841cc40f1df10ff2bbd7f9d04b7b4d401`.
The [real MRI frame](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-mri.png?raw=true)
and [revision-bound provenance](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-mri.json)
are RITK-owned evidence; Metis supplies only the format-neutral host and does
not parse DICOM data.

![Actual MRI-DIR study from the current standalone-lock replay](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-mri.png?raw=true)

### V12 fixture comparison

The recorded measurements make the comparison boundary explicit. Each row is a
real application lifecycle over public MRI-DIR data; the uncertainty is the
runner's explicitly approximate 95% half-width across three runs.

| Fixture | Input and presentation | Surface | Peak private bytes | Lifecycle |
| --- | --- | ---: | ---: | ---: |
| Métis native MIP | 409 files; axial/coronal/sagittal/axial MIP | 1280 × 800 | 2,338,119,680 ± 366,961 | 15,346 ± 3,394 ms |
| eframe | 409 files; axial/coronal/sagittal/3D MIP | 1600 × 1000 | 2,494,962,347 ± 5,291,689 | 8,752 ± 2,197 ms |
| Métis native MRI | 94 files; axial/coronal/sagittal | 1280 × 800 | 719,981,227 ± 3,928,545 | 1,955 ± 956 ms |
| eframe MRI | 94 files; axial/coronal/sagittal/3D MIP | 1600 × 1000 | 1,038,607,701 ± 2,129,690 | 2,593 ± 477 ms |
| eframe MRI orthogonal | 94 files; axial/coronal/sagittal | 1280 × 800 | 432,313,685 ± 24,884,220 | 2,070 ± 81 ms |

The [Métis MIP provenance](images/dicom-metis-real-ct-mip-resource.json),
[eframe provenance](images/dicom-eframe-real-ct-resource.json), [Métis MRI
provenance](images/dicom-metis-real-mri-resource.json) and [eframe MRI
provenance](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-eframe-real-mri-resource.json)
bind each row to its source revisions, executable, input bounds, capture digest
and sampling protocol. The eframe and Métis rows expose different MIP
semantics, surface sizes and process boundaries, including for the shared MRI
input. The orthogonal eframe row matches the three semantic planes and host extent,
while its process boundary differs. No GPUI or Tauri fixture has
been run, and WASM used memory, allocator counts, compositor latency and
security probes are separate measurements.
These rows therefore document the fixtures and their limits; they do not
establish a framework ranking.

The live browser lifecycle was then repeated in one RITK/Métis instance. The
[RITK provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-memory.json)
at commit `1e039f106cab839505be0d1739a80461c55bb9a3` records four cycles per
path after two warmups, with no reloads and a 300-second bound. Chromium chooser,
Firefox chooser and Chromium CDP drop each retained 31 host and 21 consumer
listeners while mounted, released all listeners on stop, and held committed
WASM capacity at 404,357,120 bytes after decode. Each cycle passed the file,
pixel and cine traces. These are actual saved-study application captures; RITK
owns DICOM scanning, decoding and presentation, while Métis supplies the
format-neutral host.

![Actual saved MRI study through the Métis Chromium browser path](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-cross-engine-chromium.png?raw=true)

![Actual saved MRI study through the Métis Firefox browser path](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-cross-engine-firefox.png?raw=true)

The lifecycle record bounds repeated observation; it does not establish a
long-duration leak result, allocator-used or process/compositor/GPU measurement,
or a framework ranking against GPUI, Tauri or egui.

The complete visible window for the native four-panel MIP run is also captured
from the running Windows HWND. The [RITK-owned window image](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-ct-mip-window.png?raw=true)
shows the saved public CT in axial, coronal, sagittal, and axial-MIP panels
inside the Windows frame. Two independent launches produced the same digest;
the source, executable, panel counts, dimensions, and orderly close are in the
[window provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-ct-mip-window.json).
This is a real application capture, not generated artwork. RITK owns the DICOM
decode and MIP policy; Metis owns the native window and framebuffer seam.

![Complete Métis application window showing the saved CT and axial MIP](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-ct-mip-window.png?raw=true)

The same saved public CT series was opened through the RITK browser adapter and
presented by the live Metis HTML5 canvas path. This is a runtime pixel capture,
not an illustration: RITK read all 409 Part 10 files (216,156,416 bytes),
decoded the study and supplied the axial, coronal and sagittal frames through
the format-neutral presentation boundary. The one Chromium capture used a
bounded programmatic `DataTransfer`; browser chrome is excluded, and the
capture does not claim physical drag-and-drop or cross-engine coverage.

![Actual MRI-DIR CT axial frame through the Metis browser canvas](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-orthogonal-axial.png?raw=true)

![Actual MRI-DIR CT coronal frame through the Metis browser canvas](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-orthogonal-coronal.png?raw=true)

![Actual MRI-DIR CT sagittal frame through the Metis browser canvas](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-orthogonal-sagittal.png?raw=true)

The frame dimensions, non-black-pixel counts, SHA-256 values, source
revisions and input bounds are recorded in RITK's
[`dicom-metis-real-browser-orthogonal.json`](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-orthogonal.json).
The reproducible DICOM opening and visual workflow remains in the
[RITK user manual](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md),
which owns the scanner, decoder, geometry and clinical display checks. Metis
owns the bounded file handoff, browser canvas and host lifecycle only.

The same current native host also opens the saved MRI-DIR T2 series. RITK
decoded 94 real DICOM files and transferred the axial, coronal and sagittal
MRI planes through the same format-neutral Métis framebuffer:

![Actual MRI-DIR T2 study rendered through the Métis native surface](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-mri.png?raw=true)

The run's file count, byte count, executable digest and image digest are in
[the RITK provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-mri.json).
Both captures are public MRI-DIR porcine-phantom data, not generated
illustrations or private patient studies. A private clinical path is accepted
for a local run only and is never committed to this repository. The same
three-run native workflow was sampled with Metis's resource runner; its
process-tree statistics and repeated capture digest are recorded in the
[MRI resource provenance](images/dicom-metis-real-mri-resource.json).

### Open a saved local clinical study

Use the same command with a saved study directory when you need to inspect a
real local patient image. Build `ritk-snap` from the RITK checkout, select the
acquisition's `SeriesInstanceUID` from the RITK series browser, and pass both
values to the RITK-owned launcher:

```powershell
cargo build --locked -p ritk-snap
$study = 'C:\path\to\saved-study'
$series = Read-Host 'SeriesInstanceUID shown by the RITK series browser'
target\debug\ritk-snap.exe $study `
  --series-instance-uid $series `
  --metis-native `
  --capture-application `
  --capture "$env:TEMP\ritk-metis-local-study.png"
```

The window decodes the selected files in RITK and presents the axial,
coronal and sagittal frames on the Métis surface. The optional application
capture adds the bounded RITK plane, slice, dimensions and window/level labels
to those actual decoded pixels; it is not a generated illustration. A mixed
directory must include the exact series UID, and an unknown or ambiguous
selection fails closed before pixel decode. Keep the study, UID and capture on
the local machine; do not add patient identifiers or clinical pixels to this
public repository.

Some exported studies store one image per `SeriesInstanceUID`. RITK keeps the
folder ambiguous in that case instead of guessing. To inspect one such saved
image, pass the DICOM file itself:

```powershell
$image = 'C:\path\to\saved-study\image.dcm'
target\debug\ritk-snap.exe $image `
  --metis-native `
  --capture-application `
  --capture "$env:TEMP\ritk-metis-local-image.png"
```

This direct-file path renders the real stored pixels through the same Métis
surface. A singleton image has no neighbouring slices, so its axial plane is
populated while the orthogonal planes remain empty; that is expected input
semantics rather than a fabricated placeholder. Supply a selected series when
you need a complete multi-slice MPR.

This command is the local verification path for saved-study images. The
public MRI-DIR captures above remain the reproducible repository evidence;
private studies remain local evidence only.

The current browser workflow also records a trusted window/level selection on
the same saved study. Hosted Chromium run
[35269645902](https://github.com/ryancinsight/ritk/actions/runs/35269645902)
accepted all 94 files, selected the RITK-owned `Brain T1` preset, and changed
the three displayed plane images. The [provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-window-level.json)
binds the RITK and Métis revisions, six rejected invalid probes, frame
generations, trusted input events and cleanup. The [viewport capture](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-window-level.png?raw=true)
and [control capture](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-window-level-controls.png?raw=true)
are actual RITK DICOM output through the format-neutral Métis browser surface.

![Actual saved MRI study after the Brain T1 window/level selection](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-window-level.png?raw=true)

![Window/level control for the saved MRI study](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-window-level-controls.png?raw=true)

The same saved MRI-DIR study also exercises the RITK diagnostic palette through
the Métis browser canvas. The Edge 154 trace performs 11 trusted tool actions,
rejects seven invalid tool-index probes, changes the real three-plane frame
generations and releases 21 consumer plus 32 host listeners on stop. The
[revision-bound tool provenance](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-tools.json),
[real-study capture](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-tools.png?raw=true)
and [tool palette capture](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-tools-controls.png?raw=true)
are RITK-owned clinical output; Métis remains the format-neutral canvas and
event host.

![Actual MRI-DIR study with the RITK diagnostic palette through Métis](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-tools.png?raw=true)

![RITK diagnostic-tool palette for the saved MRI study](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-tools-controls.png?raw=true)

To pair the capture with resource evidence, run the same command through
[`scripts/resource.py`](../../scripts/resource.py) as described in the
[testing manual](testing.md#measure-a-real-application-lifecycle). The report
measures the actual RITK process tree through completion and records the
initial, final and peak working set, private bytes and handle count without
copying the private command arguments into the artifact.

The same saved MRI-DIR T2 study was then opened through the packaged RITK
WASM browser path in the Codex in-app Chromium host. Métis accepted the 94 real
DICOM files as one bounded batch (49,807,236 bytes); RITK decoded them and
presented all three non-black canvases:

| Canvas | Presented pixels | Non-black pixels |
| --- | ---: | ---: |
| axial | 512 × 512 | 190,836 |
| coronal | 512 × 94 | 41,863 |
| sagittal | 512 × 94 | 38,843 |

![Actual MRI-DIR T2 axial frame through the Métis browser canvas](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-axial.png?raw=true)

![Actual MRI-DIR T2 coronal frame through the Métis browser canvas](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-coronal.png?raw=true)

![Actual MRI-DIR T2 sagittal frame through the Métis browser canvas](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-sagittal.png?raw=true)

These are live canvas exports from real DICOM decoding, not generated images.
The [RITK provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri.json)
binds the PNG hashes, source revisions and bounds. The capture excludes browser
chrome and uses a bounded programmatic drop in one Chromium host; physical
drag-and-drop, Firefox/WebKit, real WebGPU output and complete
application-window capture remain separate acceptance work. Metis now exposes
an explicit WebGPU canvas constructor; the RITK consumer must opt in and bind
the device-backed run to its own image oracle.
The reproducible DICOM opening and visual workflow is maintained in the
[RITK user manual](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md).
It runs the real scanner and loader, checks exact pixels and physical geometry,
and compares axial, coronal and sagittal captures with reviewed goldens. The
Metis browser capture proves only bounded input handoff and does not claim
DICOM decoding.
The same run records committed WebAssembly linear-memory capacity at
initialization (1,769,472 bytes), after mount (1,835,008 bytes), and after
decoding (404,160,512 bytes). The [RITK memory provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-memory.json)
contains the repeated observation and limits; it does not claim allocator-used,
browser-heap, native-process, compositor or GPU memory.

The saved MRI study was also replayed through the configured Edge WebDriver
gallery. Edge 154.0.4258.12 sent trusted file-backed drop events; the bounded
Métis host accepted all 94 files and 49,807,236 bytes, and the RITK consumer's
axial, coronal and sagittal RGBA hashes matched its independent pixel oracle.
The runner rejected count, per-file byte and batch byte overflow inputs before
reading and closed the session cleanly. The [RITK Edge gallery capture](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-edge-gallery.png?raw=true)
shows the live page and actual decoded anatomy; the [revision-bound provenance](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-edge.json)
records the transfer, images, hashes and limits. This is a Chromium-family,
file-backed WebDriver run; it does not claim physical file-manager input,
Firefox/WebKit, WebGPU or native dialog/process coverage. RITK owns the DICOM
decoder and clinical presentation; Metis owns the generic browser host and
bounded handoff.

The companion [component-state record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-components.json)
captures the same mounted page after the study is ready: 31 Rust-owned listener
handles at generation 3, the Dark theme, released pointer capture after a wheel
gesture, idle text composition and the explicit absence of an authorized
backend bridge. These observations demonstrate host/component behavior while
keeping DICOM interpretation in RITK.


## Initial form

![Initial form](images/form.svg)

The defaults are 72.50 kg, 4.00 mg/mL and 0.500 mcg/kg/min. No result exists yet.

## Successful request

![Backend result](images/form-success.svg)

The example establishes a session and submits patient `demo`, weight 60 kg,
concentration 2 mg/mL and dose 0.2 mcg/kg/min. The backend returns 0.72 mg/hour
and 0.36 mL/hour at audit sequence 2. The MAC is present but not verified by the
frontend. These synthetic values demonstrate the protocol, not treatment guidance.

## Windows host snapshots

The production form also runs in the Windows native framebuffer and packaged
WebView2 hosts. These snapshots are captured from the supervised executable,
not reconstructed images; the initial and submitted states share the same
input-sensitive backend result.

![Native framebuffer host](images/native-form.png)

![Packaged WebView2 host after submission](images/webview-form-success.png)

The complete native/WebView2 initial and submitted pairs, trusted input actions,
window sizes and SHA-256 records are in the [Windows host workflow](native.md#captured-windows-workflows)
and its [capture manifest](images/native-captures.json).

The native host also has a reviewed before/after resize pair from the same
executable: [initial](images/native-resize-initial.png),
[resized](images/native-resize-after.png), and the
[resize manifest](images/native-resize.json).

## Edit invalidates the result

![Edited form awaiting submission](images/form-edited.svg)

Changing weight to 80 kg immediately clears the old rate and MAC. No request has
been sent for these edited inputs; the form displays idle.

## Rejection and correction

![Backend rejects zero weight](images/form-rejected.svg)

The next request uses zero weight. The backend rejects it with code `0x3001` at
audit sequence 3. No prior result remains on screen. Full diagnostics are available
in the typed `FormState::Rejected` value.

![Corrected inputs produce a result](images/form-corrected.svg)

Restoring weight to 60 kg succeeds on the same session at audit sequence 4.

## Disconnection and reconnection

![Closed connection clears the result](images/form-disconnected.svg)

The example joins the finite worker, which closes the real peer endpoint. A
subsequent submission reports `0x4002`, clears the result and shows a closed
session. Synchronization uses worker completion, not a timed sleep.

![New session produces the changed result](images/form-recovered.svg)

A new app and backend session submit weight 80 kg and return 0.96 mg/hour and
0.48 mL/hour. Reconnection never silently retries an uncertain transaction.

## Reproduce and review

```text
python scripts/verify.py
```

The Rust example writes fixed `output/form*.bmp`, `output/form*.svg` and
`output/form*.csv` files. SVG paths encode the actual raster; they do not rebuild
layout as SVG text. The CSV records actual inputs, actions, state, displayed
labels and text geometry alongside independently expected outcomes.

The gate removes previous required captures before execution. It validates all
seven new captures, checks BMP/SVG pixel agreement and compares both images and
semantic records with the [reviewed baseline](images/captures.json). Source,
lockfile and rendering-fixture hashes bind the observations to this run.
For an intentional visual change:

```text
python scripts/verify.py --update-snapshots
python scripts/verify.py
```

Inspect the generated images and semantic records before accepting the baseline.
`output/visual/latest/report.json` records each comparison; adjacent expected,
actual and difference images make failures inspectable. See
[Inspect application output](testing.md) for report interpretation and retention.
Source and lock digests normalize repository text to LF, matching the committed
`.gitattributes` contract across Windows and Unix checkouts.
`output/verification.json` records the complete gate, including failures before
capture. Additional browser responsive pending/cancellation and remaining
desktop host scenarios remain in the [visual scenario contract](../VERIFICATION.md#visual-contract).

## Raster image presentation

The software display list now accepts validated raster placements for local
decoded image data. `RasterImage` rejects empty, oversized and mismatched pixel
storage; `ImagePlacement` validates an in-bounds source crop, clips an off-screen
destination and uses nearest-neighbor sampling with source-over alpha. The same
list admits clipped one-pixel line segments through `append_line` and bounded
width-aware paths through `append_polyline`, using the same painter order and
alpha compositor. Format decoding, orientation metadata
and DICOM transfer syntax selection stay with the owning Atlas provider, so this
surface can receive RITK pixels without moving medical parsing into Metis.
`ImageTransform` adds identity, axis flips, quarter-turns and validated
arbitrary affine mappings at the pixel-grid presentation boundary. The affine
form uses finite source-to-destination coefficients in normalized crop
coordinates, maps destination pixel centers through one inverse, and reads the
shared source without creating a transformed copy. Clinical orientation and
geometry still belong to RITK.

The deterministic [image example](../../examples/image.rs) renders a 3×2 color
fixture three times in a 240×180 framebuffer: unchanged, with a clockwise
quarter-turn, and through a normalized affine shear from the same source. The
display list frames the first two views with three-pixel round-cap/round-join
and square-cap/bevel-join polylines and leaves the affine sample visible below
them. The software renderer classifies each visible pixel once, which keeps
translucent overlays from darkening at segment intersections. Its generated
artifact is inspected here:

![Software raster image placement](images/image-placement.svg)

Run it with `cargo run --locked --example image`; the BMP and SVG captures are
written under `output/`. This is software-renderer evidence for V06 and covers
the format-neutral pixel-grid and normalized affine transforms plus bounded
stroke seams; it does not establish browser or native format decoding,
clinical orientation policy, fonts, media controls or native shell
presentation.

## Browser service workflow

The browser workbench has a live loopback capture path in addition to the
software gallery. Run the service command from [the browser manual](browser.md),
open the configured URL, and capture the authorized session, changed values,
`0x3001` rejection, stopped host and recovered session. The verified trace used
the Codex in-app browser at 1280×720 CSS pixels and device scale 1.25 with no
console warnings or errors. These browser captures are runtime observations;
the software gallery remains the deterministic image baseline.

## Result explorer

The browser workbench includes a bounded result explorer for applications that
receive more than one calculation response. It groups real responses by
patient, orders them by audit sequence or rate, filters patient references,
keeps selection stable across updates and exposes keyboard disclosure and
paging. The explorer is shared frontend state, so a native host can render the
same rows without adopting the browser controls. Empty, loading and typed
error states remain visible when the service is disconnected.

The current manual capture shows the empty explorer card and its semantic
controls at 1280×720; the connected-service workflow must add a live-row
capture before the RITK migration claims DICOM result-history parity. Native
value-semantic tests already cover insertion, update, eviction, filtering,
ordering, selection and tree disclosure from real typed responses.

## Framework comparison evidence

The complete comparison is maintained in
[ADR 0003](../adr/0003-framework-conformance.md). Iced 0.14 is included as a
renderer, state-model and testing comparator, and Axum 0.8 as a server/router
comparator; the current Metis build does not depend on either and this gallery
contains no Iced or Axum runtime capture. The official
[Iced examples](https://docs.rs/crate/iced/0.14.0/source/examples/README.md)
are the source reference for its native and web demonstrations. The former
[`iced_web`](https://github.com/iced-rs/iced_web) DOM runtime is archived, so
its existence does not establish HTML5/CSS compatibility for a current Iced
application or for Metis. Metis now demonstrates a bounded loopback HTTP
service over Moirai; [ADR 0025](../adr/0025-axum-server-boundary.md) defines
that first-party boundary and its public-deployment limits. Future comparator
captures require pinned, runnable fixtures and belong to their own verification
items. Svelte 5 and SvelteKit are also source comparators: Svelte's compiler,
runes and custom-element output inform the DOM/CSS migration boundary, while
SvelteKit's SSR/CSR/prerendering modes inform deployment analysis. The current
Metis build contains no Svelte dependency or JavaScript application runtime;
the real RITK MRI captures in this manual remain the visual evidence for the
Rust/WASM host. See [ADR 0003](../adr/0003-framework-conformance.md#svelte-and-sveltekit-comparison)
for the gap table and implementation order.
