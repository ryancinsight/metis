# Métis application tooling

Build an application and its explicit executable/resource inventory using the
Rust `metis` command. Portable output and Windows MSI packages use the same
validated application manifest. Moirai owns bounded build-tool processes.

```text
metis build path/to/metis.json path/to/new-output
metis package path/to/metis.json path/to/new-output
metis --help
```

See the [distribution manual](../../docs/manual/distribution.md) for the manifest,
host prerequisites, installation verification and current target limits.
