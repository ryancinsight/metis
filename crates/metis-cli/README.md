# Métis application tooling

Build an application and its explicit executable/resource inventory using the
Rust `metis` command. Portable output and Windows MSI packages use the same
validated application manifest. Moirai owns bounded build-tool processes.

```text
metis init path/to/new-app
metis dev path/to/new-app/metis.json --once
metis dev path/to/new-app/metis.json --watch
metis build path/to/metis.json path/to/new-output
metis package path/to/metis.json path/to/new-output
metis build            # browser manifest in this directory, into dist/
metis serve            # build it, then serve http://127.0.0.1:1420/
metis completions powershell > metis-completion.ps1
metis --help
```

`init` creates a dependency-free Cargo workspace, a complete locked manifest and
a runnable starter entry without overwriting an existing directory. `build`
stages a host-native portable application on Windows, macOS or Linux; `package`
adds the matching host package: Windows x64 MSI, macOS `.app` bundle or Linux
USTAR archive. `dev` uses that manifest, runs the declared
entry through Moirai and, on Windows, reloads
after real source or resource bytes change. Watch mode reports readiness, idle
and reload events; a failed Cargo run is reported and never launches a previous
executable. A manifest with a `frontend` is a browser application: `build`
compiles its package to WebAssembly and stages the page with the generated
loader, and `serve` builds it and serves it on loopback, as the
[starter](../metis-starter/README.md) shows. `completions` generates bash, fish,
PowerShell or zsh output from the same command table as `--help`.

See the [distribution manual](../../docs/manual/distribution.md) for the manifest,
developer lifecycle, host prerequisites, installation verification and current
target limits. Command documentation lives in that manual and `metis --help`. The binary does
not emit a second rustdoc tree named `metis`, which belongs to the root library.
