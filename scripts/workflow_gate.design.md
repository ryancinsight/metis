# Workflow byte classification ownership

The hosted changed-path job invokes [workflow_gate.py](workflow_gate.py). Its
`classify(bytes)` function is the sole classifier owner. Git produces NUL
separated path bytes with `--name-only --no-renames -z`; classification preserves
arbitrary bytes and embedded newlines without text decoding.

For the nonempty path records `P`, define `D` as the documented build-inert set:
case-insensitive Markdown suffixes, license names, developer Git hooks, and the
declared documentation asset suffixes beneath `docs/`. Then:

- `code = exists p in P such that p not in D`.
- `fuzz = exists p in P with the case-sensitive prefix fuzz/`.

Unknown paths retain code admission. Empty records do not create paths. Repeated
paths do not change either existential result. The production command reads
`RUNNER_TEMP/metis-changed-files` and appends the two computed lowercase booleans
to `GITHUB_OUTPUT`. Missing environment, absent input, and output errors return
a nonzero status with their original context; they never publish inferred
classification success.

The computed tests retain every previous path and fuzz vector and assert both
outputs through this production function. Real command tests exercise byte
input, append behavior, environment omissions, absent input, and output failure.
These tests replace repeated interpreter launches for pure input vectors;
they do not batch commands through a test-specific production API.

Required-job result admission remains the single inline owner in
[ci.yml](../.github/workflows/ci.yml). Its eight real command cases continue to
exercise success, skipped, failed, cancelled, draft, missing, empty, and unknown
results. The aggregate job retains `permissions: {}` and requires no checkout.

The browser runtime steps move to the canonical local reusable
[browser-runtime.yml](../.github/workflows/browser-runtime.yml). The caller
retains event admission, dependencies, fail-fast policy, matrix values, and
read-only contents permission. Four explicit inputs bind the same engine,
runner, driver endpoint, and device scale to the moved steps. All step bodies,
timeouts, actions, commands, and evidence names remain unchanged apart from
the corresponding `matrix` to `inputs` expression substitutions.

The first PR504 Windows run reaches 423 completed Python tests before the
existing 60-second suite bound expires during workflow classifier vectors.
Its log contains no individual test timings, so it does not establish a
measured Windows interpreter startup cost. This increment removes redundant
process work and embedded ownership. A subsequent actual Windows run must
prove the unchanged suite budget and the native runtime acceptance. No retry,
cache prerequisite, timeout increase, or omitted input domain substitutes for
that evidence.
