# Resolve-vs-surface disposition record — path-query

Opened at PLAN (run ec89ee4d-8358-4dd9-9eeb-e9bc59fa81c8). Closed at DECIDE.

## Domain grounding

No ungrounded domain claim. The build rests on repository-internal contracts
only: `GraphRead::traverse` returning walked edges with provenance
(`crates/wicked-estate-core/src/traits.rs:100`), the edge-direction invariant
(`docs/ENGINE-CONTRACT.md`), and the agent-behavior rules R3/R4/R5/R7.

## Assumption trio

**Files touched:** `crates/wicked-estate-core/src/query.rs`,
`crates/wicked-estate/src/{lib.rs,main.rs}`, `crates/wicked-estate/tests/`,
`crates/wicked-estate-retrieve/src/lib.rs`, `crates/wicked-estate-mcp/src/lib.rs`,
`crates/wicked-estate-mcp/tests/{sc009.rs,conformance_schemas.rs}`, and the six
documents that state a tool count.

**Tests that demonstrate done:** the per-task `Tests:` in `plan.md` — core BFS
units, the one-traversal call-count test, the CLI e2e tests, the `Path` tool
units including the 25K budget case, and the three existing tool-count
assertions reading 30/12.

**Not changing:** `GraphRead`/`GraphWrite`, any file under
`crates/wicked-estate-store/`, the conformance kit, `TraverseGraph`'s request or
response shape, and every existing tool's behaviour.

## Declined patterns

| Tempted to add | Rung | Reason |
| --- | --- | --- |
| A `path()` method on `GraphRead`, implemented across all four backends | New crate/module needs a consumer + locked storage decision (ADR-003: a backend is one module + one factory arm) | The store already returns the walked edges; a trait method would make four backends and the conformance kit change to recompute data in hand. Design decision D1 records the evidence that would reopen it. |
| `--direction dependents` on the CLI | Cut before adding — no requester | The issue asks for the forward route only. Recorded as a Follow-on, not filed. |
| k-shortest-paths / all-paths enumeration | Cut before adding — no requester | A different contract with its own output-budget problem. Recorded as a Follow-on. |
| A `PATH_QUERY_ENABLED` feature flag | §3 Feature flags are last resort | Purely additive surface; rollback is reverting the commit. No progressive rollout is real here. |
| Response truncation machinery for the R4 budget | Cut before adding — the bound already holds | `max_depth ≤ 16` bounds the hop count; an AC proves a 200-character-identifier worst case stays under 25 000 characters. Truncation code with no reachable trigger is scaffolding. |
| A second denormalization helper for `Path`'s edge endpoints | §11 Fix at the shared seam | `edge_json` already exists in `wicked-estate-retrieve`; a copy is exactly the silent-sibling drift §11 names. |

## Dispositions — pre-EXECUTE review

Five adversarial rounds, each adjudicated independently. 41 findings sustained,
9 refuted. No indeterminate.

| Round | Sustained | Refuted | Highest severity |
| --- | --- | --- | --- |
| 1 | 10 | 0 | Blocker |
| 2 | 10 | 1 | Blocker |
| 3 | 5 | 7 | Concern |
| 4 | 7 | 0 | Concern |
| 5 | 9 (not adjudicated — superseded by scope change) | — | Blocker |
| 6 | 3 | 5 | Blocker |
| 7 | 5 | 6 | Blocker |
| 8 | 6 | 1 | Concern |
| 9 | 6 | 4 | Blocker |
| 10 | 1 | 4 | Concern |
| 11 | 1 | 2 | Blocker |
| 12 | 1 | 0 | Nit |
| 13 | 1 | 0 | Blocker |
| 14 | 2 | 1 | Concern |

**Totals: 73 sustained, 36 refuted, 0 indeterminate across fourteen rounds.** The
reviewer never returned `Clean`; the loop was stopped by judgement, not by
convergence — see the stopping rationale below.

**Two design defects the review caught**, both confirmed against store internals:
`Subgraph::truncated` is never set by the depth cap, so a depth-exhausted search
would have reported a bounded absence as proven; and `SqliteStore` induces
frontier-exterior edges that `MemStore` does not, so an unfiltered BFS over
`Subgraph::edges` would answer differently per backend.

**One declination the review overturned, then restored.** I declined R4 budget
truncation at PLAN as scaffolding. Round 3 called that a blocker; the
adjudicator refuted it on evidence — `TraverseGraph`, the closest peer, also
satisfies R4 by traversal bounds alone, and the recomputed worst case is ~23.5K
against 25 000. The declination stands, now with a measurement behind it.

**Two owner scope decisions**, both taken after a recurring-failure signal:
1. The repository-wide tool-count refresh was split out, after the grep gate
   meant to prove its coverage failed three rounds running on three different
   phrasings (CLAUDE.md §10 — the third failure is evidence the model of the
   problem is wrong).
2. The scope *boundary* created by (1) was then removed entirely, after the
   `git diff` fence policing it produced two blockers of its own. This spec now
   touches no tool count and no roster, so there is no boundary to police.

**Resolved, not surfaced** (frontier rule): every finding about the removed
count-and-roster work. The accepted intent no longer requires it.


## Why the pre-EXECUTE loop stopped at round 9

Three design defects the review caught would have shipped: the depth-cap
truncation lie, the backend-divergent BFS, and the N+1 `get_node` on
`PathResult`. A fourth — that `wicked-estate-retrieve` cannot depend on
`wicked-estate`, making the planned seam unbuildable — came from reading two
`Cargo.toml` files, not from the reviewer.

By rounds 8 and 9 each revision was fixing about five findings and introducing
about two, all of the introduced ones the same species: prose describing a
structure that had moved. The highest-value remaining defects were
compile-time and behavioural, and adversarial review of prose is the wrong
instrument for those. Owner agreed to proceed to the plan gate on that basis.

**Open at the plan gate**, carried into EXECUTE rather than resolved on paper:
- Round 9's six sustained findings are applied, but no round verified the
  result. The first thing EXECUTE will test is whether the restructure and the
  new `unresolved` channel are internally consistent.
- Two owner presentation calls are reversible at the plan gate: the CLI
  `--json` `unresolved` key (versus a text-only diagnostic), and the six-field
  endpoint shape.


## Review closed at round 14 — capped by the owner, not by convergence

The owner capped the pre-EXECUTE review at round 14 regardless of outcome, after
round 13 found a live R3 defect and I had twice predicted, wrongly, that the
remaining findings were cosmetic.

**State at close.** Round 14 returned no blockers; its two sustained findings
(the `node_bounded` disjunction lacking a test, and D2 not recording the
`traverse_multi` deviation) are both applied. No unresolved Blocker or Concern
remains. But **no round verified that final state**, and the reviewer never
returned the `Clean — ready to commit.` sentinel.

**Engine consequence, stated plainly.** `loop-engine` will not leave
`SPEC-PLAN-REVIEW` without a recorded clean review, and the only recording forms
are a byte-exact clean report, a classifier-accepted clean report, or a
refuted-only adjudication. None exists. Therefore:

- `spec-approved`, `plan-approved`, `approve-plan`, `schedule` and `plan-locked`
  were **not** fired. The engine remains at `SPEC-PLAN-REVIEW`, seq 25.
- **No approved-plan baseline hash was recorded**, so the hash guard that
  normally makes `spec.md`/`plan.md` immutable during EXECUTE is not armed.
  Their immutability rests on discipline alone from here.
- `--all-skipped` was deliberately not used: `adversarial-reviewer` is mandatory
  and it ran. Recording it as skipped would have been a false record.

Both owner approvals are real and are recorded in the artifacts
(`spec.md` `Status: Approved`, 2026-09-29; `plan.md` `Status: Approved`,
2026-09-30). What is missing is the machine's countersignature, not the human's.

**Carried into EXECUTE as known-open:**
- The round-14 state is unverified by any review round.
- Two owner presentation calls remain reversible: the CLI `--json` `unresolved`
  key, and the six-field endpoint shape.
- The N-traversals-per-multi-match-name cost is a recorded risk, not a defect;
  `traverse_multi` cannot replace it (D2).
