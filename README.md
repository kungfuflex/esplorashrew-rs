# esplorashrew-rs

Esplora-compatible HTTP API as a [metashrew](https://github.com/sandshrewmetaprotocols/metashrew) WASM indexer module for [qubitcoind](https://github.com/nicholasgasior/qubitcoin).

Replaces the need for a separate Electrs/Esplora instance by running the indexer in-process as a qubitcoind secondary indexer module. All block data is indexed into an append-only key-value store and queried via view functions that return Esplora-compatible JSON.

## Overview

This crate compiles to `wasm32-unknown-unknown` and implements the metashrew indexer ABI:
- **`_start()`** — called for each new block, indexes transactions, UTXOs, addresses
- **View functions** — read-only query functions matching the Esplora REST API

## Supported Endpoints

| View Function | Esplora Endpoint | Description |
|---|---|---|
| `tx` | `GET /tx/:txid` | Full transaction details |
| `txhex` | `GET /tx/:txid/hex` | Hex-encoded raw transaction |
| `txraw` | `GET /tx/:txid/raw` | Raw transaction bytes |
| `txstatus` | `GET /tx/:txid/status` | Confirmation status |
| `txoutspend` | `GET /tx/:txid/outspend/:vout` | Output spending status |
| `block` | `GET /block/:hash` | Block metadata |
| `blockstatus` | `GET /block/:hash/status` | Block chain status |
| `blocktxids` | `GET /block/:hash/txids` | Transaction IDs in block |
| `blockheader` | `GET /block/:hash/header` | Block header hex |
| `blockheight` | `GET /block-height/:height` | Block hash at height |
| `tipheight` | `GET /blocks/tip/height` | Current chain tip height |
| `tiphash` | `GET /blocks/tip/hash` | Current chain tip hash |
| `utxosbyscripthash` | `GET /scripthash/:hash/utxo` | UTXOs by script hash |

## Building

```bash
# Native (for testing)
cargo build

# WASM (for deployment)
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

## Testing

Tests follow the [alkanes-rs](https://github.com/kungfuflex/alkanes-rs) / metashrew-core pattern: an in-memory `CACHE` HashMap replaces the host extern functions in native test builds, so the full indexer pipeline (block parsing, indexing, view queries) runs natively without a WASM runtime.

```bash
cargo test -- --test-threads=1
```

Single-threaded execution is required because the in-memory cache uses global mutable state (same constraint as `metashrew-core`).

### Test coverage

- **Block parser** (12 tests) — coinbase/spending tx parsing, header parsing, txid/hash computation, compact size encoding, weight calculation
- **Indexer** (8 tests) — block/tx metadata, raw tx storage, block txid lists, spend records, UTXO creation, address-tx mappings, multi-block tip progression
- **View functions** (17 tests) — all 13 view functions tested end-to-end including not-found cases and a full multi-block integration test

## Usage with qubitcoind

```bash
# Install the indexer
qubitcoin-cli installindexer esplora target/wasm32-unknown-unknown/release/esplorashrew.wasm

# Start qubitcoind with the indexer
qubitcoind -loadindexer=esplora:~/.local/qubitcoin/indexers/esplora

# Query via RPC
qubitcoin-cli secondaryheight esplora
qubitcoin-cli secondaryview esplora tipheight ""
qubitcoin-cli secondaryview esplora blockheight "$(printf '%s' '100' | xxd -p)"
```

## Architecture

```
qubitcoind
├── chain sync (P2P)
├── chainstate (UTXO set)
└── secondary indexers
    └── esplorashrew.wasm
        ├── _start()     → index block into KV store
        └── view_*()     → query indexed data (Esplora JSON)
```

The WASM module uses the metashrew host ABI:
- `__host_len()` / `__load_input()` — read input data
- `__get()` / `__get_len()` — read from append-only KV store
- `__flush()` — write key-value pairs atomically
- `__log()` — structured logging

## License

MIT
