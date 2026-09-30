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

## Post-gates review round 1 (2026-09-30)

Two reviewers ran against the implementation diff; both reports were adjudicated
independently. `adversarial-reviewer`: 9 sustained / 1 refuted. `quality-engineer`:
12 sustained / 3 refuted.

### The one live defect

**MCP `Path` with `depth: 0` claimed a proven absence.** The CLI rejected `--max-depth 0`;
the MCP surface clamped only the upper bound, so a request with `depth: 0` expanded nothing,
left `depth_bounded` false (no backend records the start in `depths`), and emitted "the whole
reachable set was searched, so no route exists" — the R3 failure this feature is built
around, on the surface agents actually call. Fixed by flooring to 1 and pinning `minimum: 1`
in the advertised schema and the frozen golden.

### What the coverage audit found, and what it says about the earlier claim

The quality reviewer mutation-tested the parts of the diff I had not, and found four
load-bearing mechanisms whose removal left the entire suite green:

| Mechanism | Mutation that survived |
| --- | --- |
| MCP dispatch arm | deleting `\| "Path"` from `handle_request_unified_ro` |
| `depth` clamp | removing `.min(16)` |
| `max_nodes` clamp | removing `.min(5_000)` |
| BFS adjacency sort | replacing it with `admissible.reverse()` |

Each now has a falsifier, and each mutation was re-run to confirm it dies:

- Dispatch arm → `path_dispatches_through_the_unified_arm_in_both_modes` (also discharges the
  read-only half of the criterion). The prior test used `handle_request`, which resolves
  against `all_tools()` and never reads the arm the binary uses.
- `depth` clamp → `path_depth_clamp_is_falsifiable` (a 20-hop chain must NOT be found at
  `depth: 99`; the old fixture was 16 hops, found either way).
- `max_nodes` clamp → `path_node_budget_clamp_is_falsifiable` (a 6 000-leaf hub).
- Adjacency sort → `hop_sequence_is_independent_of_edge_and_node_order`. The old
  determinism test called `shortest_path` twice on the *same* `Subgraph`, which any pure
  function satisfies.

**The lesson for the record.** T2 and T4 claimed mutation verification, and that claim was
true but narrow: the two tests checked were the two written as falsifiers. Sampling only the
tests designed to fail said nothing about the rest, and "1484 passing" was reported with more
confidence than it had earned. Mutation-test the mechanism you are relying on, not the test
you are proud of.

### Also repaired

- `CHANGELOG.md` stated "Estate tools 11 → 12, total 30" while `README.md` still says 29 —
  the cross-document contradiction the owner's scope split existed to prevent, and a
  violation of this spec's own no-count criterion. Count clause removed.
- Two stale count comments in `wicked-estate-mcp/src/lib.rs`, plus the pre-existing
  "advertises all 28" line, which was already wrong before this change and is one line.
- `to`-side unresolved rendering, zero-hop identity route, and the R7 boundary at exactly
  0.5 (`ResolutionTier::Heuristic`) — all previously unexercised; the `to` branch collapse
  was mutation-confirmed to survive the old tests.
- Endpoint completeness now asserted on `SqliteStore` at the depth frontier, the backend the
  admission rule was written for; the previous assertion ran only on `MemStore`.
- `EdgeKind` gained a derived `Ord` so the adjacency tie-break stops allocating two `String`s
  per comparison. The alternative — dropping the tie-break — was explicitly rejected: it is
  cheaper and silently destroys the determinism the sort exists for.
- Endpoint selection uses a `HashSet` rather than a linear `Vec::contains`.
- The CLI spawn fixture now cleans up after itself.

### Refuted, and why it matters

- **Unbounded candidate fan-out** (one traversal per `from` candidate, no cap). Refuted: the
  plan's Risks section records it as an accepted residual risk, and a cap would silently
  convert a proven absence into an unreported bounded one — the R3 failure the design is
  built around. Reopening needs the plan's own trigger: an integrator reporting a slow
  `path` on a large graph.
- The CLI span's attributes, and the two hand-delegating `GraphRead` test doubles: no
  criterion, rule or defect named.
- The spec's unchecked criteria and `Status: Approved`: finish-checklist work not yet due at
  review time, not a defect in the reviewed target.

## Post-gates review round 2 (2026-09-30)

`adversarial-reviewer`: 5 sustained / 3 refuted. `quality-engineer`: 5 sustained / 2 refuted
(after a re-adjudication — see below).

### The R3 class, and why it took three rounds

Round 1 fixed MCP `depth: 0` emitting a proven-absence claim. Round 2 found the same class
one branch over: an operand that is **present but empty** (or not a string) short-circuited
before `path_between` and returned `{found:false, depth_bounded:false, node_bounded:false,
unresolved:null}` — byte-identical to the shape the spec licenses a reader to treat as "no
route exists" — while the diagnostic called a present operand "required". The CLI handled the
same input correctly, so the two surfaces disagreed.

That is a CLAUDE.md §11 failure on my part: a fix in one place is a hypothesis about all the
others, and I applied the `depth` fix without re-auditing the sibling early returns.

**Fixed at the seam, not the site.** Every no-route reply in the `Path` tool now goes through
one constructor that *requires* the reason:

```rust
let no_route = |unresolved: Option<&str>, diag: Vec<String>| …
```

A branch can no longer hand-build `unresolved: null` for a case that has not earned it. That
is what stops a round 3 from finding a fourth instance.

Round 2 also found the same property unpinned on the **text** surface: every unresolved-vs-
absence test went through `--json`, so collapsing the text branch into a bare "no path found"
was free. `print_path_text` became `write_path_text(&mut impl Write, …)`, because two of its
branches — the node-budget line and the proven-absence coverage line — cannot be provoked
through a spawned binary at all (D7 fixes the CLI budget at 5 000), and the module doc
claimed they were asserted in-process when nothing called the function.

### Falsifiers added, each mutation-confirmed to die

| Mechanism | Mutant | Test that now dies |
| --- | --- | --- |
| shortest across `from` candidates | `best.is_none()` | `shortest_route_wins_across_from_candidates` |
| edge-kind tie-break (and the `EdgeKind` `Ord` derive) | drop `.then_with(…kind…)` | `parallel_edges_of_different_kinds_resolve_deterministically` |
| text unresolved branch | collapse to "no path found" | `text_unresolved_operand_does_not_read_as_a_proven_absence` |

The first two had no fixture of the right *shape*: every multi-match test had one routing
candidate or two of equal length, and every fixture built `Calls` edges only, so the
comparison and the tie-break decided nothing observable.

### The `max_nodes` floor is defensive, not load-bearing

Adjudicated advisory, and the proposed test repair was ruled **wrong**. Tracing `max_nodes: 0`
through all three backends: `MemStore` sets `truncated` at the first neighbour
(`sub_nodes.len() >= 0`), `SqliteStore` issues `LIMIT 0` so `depths.len() >= 0` holds, and
`PostgresStore` fetches `max_nodes + 1` and flags `raw.len() > 0`. In every case
`node_bounded` is true, so the reply is an honest bounded absence and R3 holds. The floor is
therefore not the `depth: 0` defect's sibling — depth 0 genuinely produced a false proven
absence and `max_nodes: 0` does not. No test at the designated `MemStore` seam can kill the
floor, because 0 and 1 are observationally identical there. Recorded rather than pinned.

### An invalid adjudication, and what was done about it

The first `quality-engineer` adjudication emitted a bare `ADJUDICATION-INDETERMINATE` token
and retracted it in prose. `review inspect` classified it `invalid (indeterminate-present)`,
which is a fail-closed stop; the bounded evidence-retry route needs a pre-declared gate
catalog this repository does not have. Its findings were legible and I agreed with them,
which is exactly why acting on them would have made the gate decorative. The owner chose
re-adjudication; a fresh verdict over the unchanged findings came back valid, and two of the
reviewer's seven findings were refuted on that pass.

### Refuted, and kept refuted

- **Candidate fan-out** (third time): the plan's Risks records it, and a cap would collide
  with the shortest-over-every-pair criterion.
- **R4 crossover at ~216 chars**: the arithmetic is right, but reaching it needs symbol ids
  *and* names *and* file paths to average >215 characters simultaneously; with realistic
  names (~40) and repo-relative paths (~80) the ids would need ~490, far beyond what
  `Symbol`'s `Display` produces. A measured identifier-length distribution would be the new
  evidence that reopens it.
- **`EdgeKind` absent from `What Changes`**: the Interface-compatibility row is scoped to
  tools, traits and schemas with a closeout of `traits.rs` and `store/src`; an additive
  derive touches neither.
- **`ids.dedup()` as dead code**: `GraphRead::find_symbols` documents no uniqueness
  guarantee, so removing it at a `&dyn` seam is unsafe for an out-of-tree store.
