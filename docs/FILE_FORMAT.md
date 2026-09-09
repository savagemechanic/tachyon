# Tachyon experiment file

Version 1 is little-endian binary: 8-byte `TACHYON\0` magic, `u16` version, `u64` body length, body, and an FNV-1a 64-bit integrity checksum. The body contains seed, event count, initial price, tick size, price count, price ticks, and result hash. Readers bound lengths before allocation and reject bad magic, unknown versions, truncation, trailing fields, oversize counts, and checksum failures. This checksum detects corruption; it is not cryptographic authentication.
