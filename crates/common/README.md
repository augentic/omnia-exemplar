# Common

Shared domain logic used by multiple crates. Provides clients for the Block
Management API (`block_mgt`) and Fleet API (`fleet`) — retrieving vehicle
allocations and vehicle metadata respectively.

Both clients are written against the `omnia-guest` capability traits
(`Config`, `HttpRequest`, `Identity`) so the same code runs inside the WASM
guest and against native mock providers in tests.
