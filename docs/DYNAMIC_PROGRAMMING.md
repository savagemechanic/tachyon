# Dynamic programming

Tachyon represents state transitions as a weighted directed graph. Each state evaluates HOLD, passive buy/sell, and aggressive buy/sell edges in a deterministic action order. Direct tabulation selects the maximum state-action value; equal rewards use the declared action order. Tiny analytical graphs, negative rewards, terminals, and invalid edges are contract-tested. No RL framework is used.
