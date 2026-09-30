# Verification ledger — path-query

Execution observations. Not the contract; `spec.md` and `plan.md` are.

## T3 — manual QA of the real binary (2026-09-30)

Fixture: a four-function Rust chain indexed into a scratch db —
`http_handler → service_create → repository_save → db_write`, which is issue #192's
own question ("how does this HTTP handler reach the database write?").

**Text mode** — `wicked-estate path http_handler db_write --db graph.db`, exit 0:

```
3 hop(s) from 'http_handler' to 'db_write':
  http_handler (src/app.rs:4) -> service_create (src/app.rs:3)  [Calls] confidence 0.65 (scoped-name-resolver)
  service_create (src/app.rs:3) -> repository_save (src/app.rs:2)  [Calls] confidence 0.65 (scoped-name-resolver)
  repository_save (src/app.rs:2) -> db_write (src/app.rs:1)  [Calls] confidence 0.65 (scoped-name-resolver)
```

**`--json`** — one document, `found: true`, three hops, each endpoint carrying all six
fields (`symbol`, `name`, `kind`, `file`, `line`, `line_1based`), each hop carrying
`confidence`, `provenance` and `resolved_by`. No staleness notice on stdout.

### Observation that changed the code

The first working version printed raw `SymbolId`s
(`ts-rust . . . src/app/http_handler(). -> ts-rust . . . src/app/service_create().`).
That satisfies the acceptance criterion as written — the line does name the source symbol,
target symbol, kind and confidence — but it defeats the point of the feature, which is that
a reader can *name* the intermediate functions and open them. T3's Done-when marks column
layout as build-time detail, so text mode now renders `name (file:line)` from
`PathResult::endpoints`, at no store cost. The four required facts are unchanged.

A passing unit gate would not have caught this; only running the binary did.

### Incidental, not caused by this change

`index` emits `EMIT-DEADLETTER: … spawn 'wicked-bus' failed` on a machine with no
`wicked-bus` binary, and spools to the dead-letter file. Pre-existing behaviour of the
event bus, unrelated to `path`.
