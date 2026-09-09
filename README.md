# Tachyon

**A first-principles market simulation laboratory for high-frequency trading research.**

Tachyon explores market microstructure from fundamental state transitions—not from candles, machine-learning frameworks, or trading libraries. The project begins as a deterministic research laboratory in Rust, with a standard-library-only simulation core and explicit invariants for every important structure.

> In Tachyon, time is not initially the clock. The market itself is the clock.

An integer event sequence measures market evolution:

```text
0 -> 1 -> 2 -> 3 -> ...
```

The kernel is built around one transition:

```text
S[n + 1] = F(S[n], E[n])
```

Wall-clock timestamps can later be layered on as optional replay metadata without becoming fundamental to the state machine. Latency can first be modeled as event distance: an action becomes visible after a defined number of intervening market events.

```text
Market State
     │
     │ Event
     ▼
Transition
     │
     ▼
New Market State
     │
     ├────► metrics
     │
     ├────► agents
     │
     └────► next event
```

## Principles

- Correctness and measurement before optimization.
- Strict test-driven development, one invariant and milestone at a time.
- Integer representations for prices, quantities, sequence numbers, and other discrete market values whenever exact arithmetic is possible.
- Deterministic results from the same seed, initial state, event sequence, and configuration.
- Standard-library data structures before exotic alternatives: `BTreeMap`, `HashMap`, `Vec`, and `VecDeque`.
- Explicit invariants and expected time and space complexity for important structures.
- No frameworks, unnecessary dependencies, premature abstractions, or layered service/repository/controller/manager architecture.

No third-party Rust crate will enter the simulation core without a documented reason and explicit approval.

## Planned foundation

The first meaningful implementation milestones are primitive market types, a tagged `MarketEvent`, an append-only ordered event log, FIFO price levels, and a price-time-priority order book. A matching state machine, balances, simulation state, deterministic pseudo-random generation, agents, metrics, replay, and market-specific research follow only after the earlier invariants are tested.

Initial complexity expectations include:

| Operation | Expected complexity |
| --- | --- |
| `BTreeMap` price lookup | `O(log n)` |
| `BTreeMap` insert/remove | `O(log n)` |
| `HashMap` order lookup | expected `O(1)` |
| `VecDeque` front/back | amortized `O(1)` |
| event-log append | amortized `O(1)` |

The design philosophy draws on the invariant-first and complexity-conscious principles of *Introduction to Algorithms*, *The Algorithm Design Manual*, *Guide to Competitive Programming*, *Test-Driven Development: By Example*, and *Growing Object-Oriented Software, Guided by Tests*, adapted to Rust and explicit state machines rather than forced object-oriented patterns.

## Development

Every stage is a TDD gate:

```text
requirement
    │
    ▼
invariant
    │
    ▼
failing test
    │
    ▼
minimum implementation
    │
    ▼
passing test
    │
    ▼
refactor
    │
    ▼
property / adversarial tests
```

Run the foundation checks with:

```sh
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

Work should proceed milestone by milestone on branches through pull requests rather than accumulating directly on `main`.

## Project status

Tachyon is currently at **stage 0: repository and CI foundation**. No order-book, matching, replay, agent, strategy, live-exchange, or market-specific functionality has been implemented yet.

Tachyon is a simulation and research project. It is not a live-money trading bot, financial advice, or a promise of profitable trading.
