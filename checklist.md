# Metis dependency repair

Target version: 0.3.2. Phase: Closure.

- [x] [patch] Compare consumed Consus APIs with the canonical locked revision.
- [x] [patch] Correct both requirements to 0.2 and synchronize package versions.
- [x] [patch] Verify consumed Moirai WebView APIs and admit current version 0.2.
- [x] [patch] Regenerate the canonical Git lock and verify locked resolution.
- [x] [patch] Run workspace/all-target Clippy and all 686 existing nextest tests.
- [x] [patch] Verify fresh update dry-run, formatting and diff consistency.
- [x] [patch] Record verification and keep RITK dependency changes separate.
