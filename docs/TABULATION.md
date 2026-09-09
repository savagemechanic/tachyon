# Tabulation

Bounded feature buckets form a tuple. Mixed-radix encoding maps it bijectively to a contiguous `usize` index. Invalid dimensions and buckets fail rather than clamp. Each cell retains support and direction counts, signed/absolute/squared distances, and extrema, allowing exact integer estimates without a hash-table iteration order.
