# Architecture

```text
typed events -> order book/state machine -> MarketState -> Impact
                                                  |          |
                                                  v          v
                                            mixed-radix   strategy
                                               table       policy
                                                  \          /
                                                   simulation
                                                       |
                                             binary result + dashboard
```

The v0.1 core uses only Rust's standard library. Data and algorithms remain explicit: `BTreeMap<Price, PriceLevel>` gives `O(log n)` price operations; `VecDeque<Order>` gives amortized `O(1)` FIFO ends; `HashMap<OrderId, ...>` gives expected `O(1)` active-order lookup; `Vec<ImpactCell>` gives `O(1)` dense table lookup after `O(d)` mixed-radix encoding. Networking is a loopback-only standard-library HTTP server with embedded HTML and SSE.
