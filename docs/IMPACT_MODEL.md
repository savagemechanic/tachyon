# Impact model

Each experiment explicitly chooses midpoint, microprice, best bid, best ask, or last trade. Targets are the next price-changing event, exactly the next `N` events, or the first move of at least `K` ticks. Output is direction `-1/0/+1`, signed integer ticks, and integer event distance. Missing references or unreached targets fail explicitly.
