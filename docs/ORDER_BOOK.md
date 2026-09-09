# Order book

The book is a B-tree of prices containing FIFO ring queues, plus a hash index by order ID. Best bid is the last bid key and best ask the first ask key. Adds reject duplicate IDs and crossed resting orders. Matching repeatedly selects the opposite best price and consumes queue fronts; partial quantities update cached totals and full fills remove index entries and empty levels. Price operations are `O(log n)`, indexed lookup expected `O(1)`, and FIFO ends amortized `O(1)`.
