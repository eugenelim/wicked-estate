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


## T4 — measured R4 worst case (2026-09-30)

The acceptance criterion pins a 16-hop route whose symbol ids, names and file paths are each
200 characters, measured over the content block plus the diagnostics block the MCP layer
appends. Measured: **content 23 423 + diagnostics 102 = 23 525** of the 25 000-character
budget — 5.9% headroom.

This is the number that settled a round-3 finding which called the budget a blocker and
proposed adding `cap_rows_to_budget` truncation to `Path`. The adjudicator refuted it
(TraverseGraph, the closest peer, also satisfies R4 by traversal bounds alone) and estimated
~23.5K; the measurement confirms it. The `max_depth <= 16` clamp is the enforcement
mechanism, and there is no truncation step to maintain.

## T4 — mutation checks (2026-09-30)

Two tests in this change exist to fail against a plausible wrong implementation. Both were
verified by mutating the source and confirming they die:

- Removing the `edge_json` cache seeding from `PathResult::endpoints` fails
  `path_renders_a_16_hop_route_with_zero_get_node_calls` and nothing else.
- Swapping the bound-flag disjunction (`|=` to `=`, i.e. last-candidate semantics) fails
  exactly `depth_bounded_is_a_disjunction_across_candidates` and
  `node_bounded_is_a_disjunction_across_candidates` in the store crate.

The disjunction tests needed a fixture change to earn that: in the first draft the bounded
candidate happened to sort last, so last-candidate semantics gave the right answer by luck
and the tests passed against the mutant. The bounded candidate now sorts first.
