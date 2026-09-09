# Market model

Tachyon treats market activity, not wall time, as the primary clock. `MarketState + MarketEvent -> MarketImpact(direction, price distance, event distance)`. Price is integer ticks, quantity integer lots, probability basis points, and sequence/event distance `u64` events. Midpoint and microprice use half-tick integer units so half ticks remain exact. Crypto and YES/NO prediction instruments share the state machinery without exchange-specific networking.
