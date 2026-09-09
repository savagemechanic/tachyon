# Tachyon

**A first-principles market simulation laboratory for high-frequency trading research.**

Tachyon models typed market events and compact market state, extracts impact as direction × price distance × event distance, learns explainable tabulated rules and policies, and validates them through deterministic simulation. The market—not wall-clock time—is initially the clock.

```text
Market Event + Market State
            │
            ▼
        Market Impact
            │
      ┌─────┼─────┐
      ▼     ▼     ▼
 direction distance activity
```

## Install and quick start

Download the archive for your platform from [Releases](https://github.com/savagemechanic/tachyon/releases), verify it against `SHA256SUMS`, extract it, and place `tachyon` on your `PATH`.

```sh
tachyon --help
tachyon simulate --seed 42 --events 1000 --output experiment.tachyon
tachyon inspect experiment.tachyon
tachyon replay
tachyon extract experiment.tachyon
tachyon strategy experiment.tachyon
tachyon experiment experiment.tachyon
tachyon benchmark 100000
tachyon dashboard --port 7878
```

Open `http://127.0.0.1:7878` for the embedded, local-only research dashboard. It shows experiment identity, market state, direction support, strategy performance, inventory, event summary, and the extracted rule/policy, with SSE live status.

## What is implemented

- Typed tick/lot/basis-point/event-sequence units and explicit `MarketEvent` variants.
- Price-time-priority CLOB using `BTreeMap`, `VecDeque`, and an indexed `HashMap`.
- Compact market state, midpoint/microprice/depth imbalance, and explicit impact targets.
- Bijective mixed-radix compression and contiguous impact statistics.
- Direct tabular dynamic-programming policy selection.
- Deterministic xorshift generation, replay, accounting, strategy reports, crypto instruments, prediction contracts, and cross-market divergence.
- Versioned, bounded, checksummed binary persistence.
- Standard-library CLI, HTTP dashboard, and SSE; no third-party runtime dependencies.

## Reproducibility and testing

The same dataset bytes, configuration, seed, definitions, and Tachyon version produce the same result hash and persisted output. The immutable [testing contract](docs/TESTING_CONTRACT.md) covers invariants, hostile traces, anti-lookahead, accounting, DP, replay, generation, persistence, CLI, dashboard, and release acceptance.

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

CI runs these checks on Linux and macOS. The deterministic property harness executes thousands of events across known seeds. `tachyon benchmark` establishes a local throughput baseline; optimization requires before/after evidence.

## Design documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Market model](docs/MARKET_MODEL.md)
- [Order book](docs/ORDER_BOOK.md)
- [Impact model](docs/IMPACT_MODEL.md)
- [Tabulation](docs/TABULATION.md)
- [Dynamic programming](docs/DYNAMIC_PROGRAMMING.md)
- [File format](docs/FILE_FORMAT.md)
- [Releases](docs/RELEASES.md)

Tachyon is a simulator and research environment, not a live-money trading bot, financial advice, or a promise of profitable trading. Live exchange connectivity, production calibration, Hawkes processes, and venue-specific fee schedules remain future research rather than implied capabilities.
