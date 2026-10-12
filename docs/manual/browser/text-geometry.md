# Browser text geometry

[Browser manual](../browser.md)


## Measure browser text geometry

The workbench can record the browser's actual line and grapheme layout for the
**Clinical note** editing surface. Build the generated page, then run the
format-neutral workbench trace with the option below (the WebDriver URL is the
same one used by the other browser traces):

```text
python scripts/browser.py build
python scripts/browser_runtime.py --engine chromium --serve-dir output/browser \
  --text-geometry-probe --lifecycle-cycles 4 \
  --output output/browser/runtime/chromium-text-geometry.json
```

The probe copies the textarea's computed font, direction, writing mode, padding
and bounded width into a detached same-origin element. It measures a fixed
Unicode specimen containing a combining mark, CJK characters, Hebrew, a joined
emoji, a newline and a second line. `Intl.Segmenter` supplies the grapheme
boundaries and `Range.getClientRects()` supplies the host line and cluster
fragments. The JSON trace records the UTF-16 offsets, finite cluster bounds,
line assignments, `visual_order` grapheme-start permutation, line-top
ordering, computed style and the live textarea's selection/scroll metrics.
When one grapheme range spans a line boundary, visual bounds and line
assignment use the non-zero-width fragments, choosing the fragment with the
greatest `top` coordinate and falling back to all fragments when they are
zero-width). This keeps newline, bidi and boundary ranges in the line where
the glyph is rendered across `Range.getClientRects()` implementations instead
of treating a zero-width boundary as a separate first-line glyph.
It also records bounded `CanvasRenderingContext2D.measureText()` widths for
Latin, combining-mark, CJK, Hebrew and joined-emoji samples, together with the
effective canvas font string and the `FontFaceSet` loading status/check. A
detached span set measures the explicit `system-ui`, `sans-serif`, and
`monospace` fallback chain, recording requested and computed family strings,
finite widths/heights and the same loading checks. These measurements make the
browser's current text-width and fallback-chain behavior inspectable without
claiming that the browser exposes the identity of every selected system face.
The validator requires the visual order to be a complete permutation of the
grapheme starts and rejects duplicate entries or clusters assigned outside the
measured line set. The element is removed before the command continues, so
this probe does not mutate the application value or add a screenshot surface.
When `--lifecycle-cycles` requests remounts, the runner also compares the
observations' bounded text contract: fixture and UTF-16 grapheme boundaries,
visual order, line count, direction, writing mode and the requested/computed
font-chain identity must remain stable. Host rectangle and width values remain
per-observation measurements because rasterization can vary; the stability
check does not turn them into a cross-engine equality claim.

An engine that lacks `Intl.Segmenter` records an explicit unavailable reason;
the runner never substitutes scalar or pixel estimates. These measurements are
browser layout and font-metric evidence for the HTML5/CSS path. They do not
establish native IME delivery, selected fallback-face identity, clipboard permissions,
assistive-technology behavior or pixel identity across engines.

### Compare text semantics across engines

The scheduled matrix also downloads the three workbench traces and runs the
format-neutral comparator:

```text
python scripts/browser_text_cross_engine.py \
  --trace chromium=output/browser/runtime/chromium-workbench.json \
  --trace firefox=output/browser/runtime/firefox-workbench.json \
  --trace webkit=output/browser/runtime/webkit-workbench.json \
  --output output/browser/runtime/text-cross-engine.json
```

The comparator requires passed schema-1 traces from all three configured engines
and at least two available text-geometry observations. It compares the fixture,
UTF-16 length and grapheme boundaries, visual-order permutation, line count,
direction and writing mode. Rectangle coordinates, measured widths and the
selected fallback face remain per-engine records; a numeric range is not turned
into a false pixel-equality claim. Missing or unavailable engine geometry fails
the comparison with its explicit reason, so the hosted matrix cannot silently
report a partial cross-engine result.

Hosted manual run [35368255350](https://github.com/ryancinsight/metis/actions/runs/35368255350)
passed the Chromium, Firefox and WebKit workbench traces and the comparator.
The published `metis-browser-text-cross-engine-35368255350` artifact records
five observations per engine and the shared fixture, UTF-16 length, grapheme
boundaries, visual order, line count, direction and writing mode. The runtime
artifact upload roots each trace at `output/browser/runtime`; this avoids a
workspace-prefix extraction path that would otherwise fail before comparison.

[Input Events]: https://w3c.github.io/input-events/
