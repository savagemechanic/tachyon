# Tachyon Immutable Testing Contract

This document is the authoritative behavioral contract for Tachyon v0.1.0. It is organized around externally observable invariants, not implementation choices. After production implementation begins, a failing contract test must be fixed in production code. A contract may change only to resolve a demonstrated internal contradiction, with a documented reason and an equally strong or stricter replacement.

## Units and arithmetic

- `Price` is a non-negative integer count of instrument ticks (`u64`).
- `Quantity` is a positive integer count of lots (`u64`) for live orders and fills.
- `Probability` is an integer count of basis points in `[0, 10_000]`.
- `Sequence` and event distance are counts of market events (`u64`), not clock time.
- Cash, fees, rebates, reward, P&L, midpoint, microprice, and distances use signed fixed-point integer arithmetic with documented scales. Overflow and invalid conversion fail explicitly.

## Order book

- Outside the transient execution of a match, a two-sided book is not crossed: `best_bid < best_ask`.
- Best bid is the greatest bid price; best ask is the least ask price.
- Orders at one price execute strictly FIFO.
- Active order IDs are globally unique.
- Cancelling an active order removes exactly that order; cancelling an unknown order fails explicitly.
- A partial fill reduces exactly the target order and price-level quantity by the fill quantity.
- A full fill removes the order and index entry.
- Empty price levels are removed.
- Each price level's cached total equals the sum of its contained remaining quantities.
- Price lookup/insert/removal is `O(log n)`, indexed order lookup is expected `O(1)`, and queue front/back is amortized `O(1)`.

## Conservation and accounting

- A match creates identical maker and taker traded quantity; traded quantity is never silently created or destroyed.
- Each fill changes buyer and seller asset positions by equal and opposite quantities.
- Cash transferred at the execution price, plus explicitly recorded fees and rebates, reconciles exactly.
- P&L equals realized cash flow plus terminal inventory valued at the documented mark price, less fees plus rebates.
- Position limits are enforced before an action becomes an accepted fill.

## Market state and events

- Events use an explicit tagged union with typed payloads; unknown/stringly typed payload maps are not accepted.
- State exposes the units and reference price for every value.
- Spread, midpoint, microprice, depth, flow, cancellation pressure, queue state, inventory, and market-specific values are deterministically derived from the same ordered observations.
- Invalid zero quantities, duplicate IDs, impossible replacements, invalid settlement, and malformed states fail explicitly without partial mutation.

## Market impact

- Every impact result names its reference price and target definition.
- Known traces yield exact direction in `{-1, 0, +1}`, signed price distance in ticks, and event distance.
- Supported targets are next price-changing event, next `N` events, and first movement of at least `K` ticks.
- No experiment silently mixes midpoint, microprice, best bid, best ask, or last-trade references.

## Tabulation and compression

- Mixed-radix encoding and decoding are bijective across every valid state tuple.
- Distinct valid tuples never share an ID.
- Invalid bucket values fail explicitly; they are never silently clamped.
- Counts, direction counts, signed/absolute/squared distances, extrema, means, and variance match hand-calculated fixtures.
- Dense bounded tables use contiguous cells and deterministic iteration.

## Dynamic programming

- Tiny finite state graphs produce analytically known optimal policies.
- Terminal states, unreachable states, negative rewards, inventory-dependent choices, and deterministic tie-breaking are explicit.
- A policy decision uses only its current state and available transitions.

## Strategy and execution

- A strategy decision cannot access an event or state that occurs after its decision sequence.
- Explicit anti-lookahead fixtures fail if future information leaks into features, rules, or policy lookup.
- Passive and aggressive actions model no fill, partial fill, full fill, queue ahead, fees/rebates, slippage, adverse selection, inventory, and position limits.
- Reports include observations, expected impact, expected execution reward, fill rate, wins, losses, mean reward, variance, maximum drawdown, inventory behavior, support, and out-of-sample results.

## Determinism, replay, and generation

- Same input dataset bytes, configuration, seed, feature/target/strategy definitions, and Tachyon version produce byte-identical persisted results and the same result hash.
- Known event traces produce exact final snapshots.
- Synthetic runs reproduce exactly from their seed.
- The deterministic property harness checks all invariants after every transition and reports seed, iteration, state, and event on failure.

## Persistence and malformed input

- The Tachyon binary format is versioned, length-bounded, checksummed, and deterministic.
- Valid experiment and replay files round-trip exactly.
- Corrupt, truncated, oversized, and unknown-version files fail cleanly before untrusted allocation.
- Repeated settlement and invalid configuration fail explicitly.

## CLI, dashboard, and release

- `tachyon` and `tachyon --help` explain the available research flows.
- Inspect, replay, simulate, extract, strategy, experiment, benchmark, and dashboard flows produce concise readable output and nonzero exit codes on failure.
- The dashboard binds to loopback by default, exposes useful research state, handles empty/error states, and has working controls and live updates without runtime console errors.
- CI runs formatting, strict Clippy, all-target tests, and release builds on Linux and macOS.
- Published artifacts include checksums. A downloaded artifact must verify, install, run help, deterministic simulation, replay, extraction, tabulation/DP, persistence, dashboard, restart, and result reload equivalently to the source build.

## Adversarial cases

The suite must cover zero quantity, duplicate IDs, unknown cancellation, extreme price/quantity, empty and one-sided books, rapid add/cancel, partial-fill chains, full-depth sweeps, crossed books, repeated settlement, invalid configuration, bucket boundaries, corrupt/truncated/unknown binary files, and allocation bounds. No hostile trace may silently produce invalid state.
