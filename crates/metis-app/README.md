# metis-app

One application executable composes the backend and presentation libraries.
The parent owns session keys, authorization and audit state; it relaunches its
own executable through Moirai with a presentation role and private pipes.
Both processes contain the same compiled code, but hold separate process state.

```powershell
cargo run --locked -p metis-app -- 60 2 0.2
```

The console demonstration prints a flow of 0.36 mL/hour and a drug rate of
0.72 mg/hour. `--help` prints invocation syntax. The internal
`--metis-frontend` argument selects a child role; it grants no authority.
Windows job containment bounds the managed session, not OS permissions.
Native GUI hosting and other platform containment remain separate capabilities.

See the [user manual](../../docs/manual/distribution.md) for portable and
installer workflows and the [application decision](../../docs/adr/0006-application-entry.md)
for process boundaries and migration from the removed demonstration commands.
