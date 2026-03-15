# esplorashrew-rs

Esplora-compatible HTTP API as a [metashrew](https://github.com/sandshrewmetaprotocols/metashrew) WASM indexer module for [qubitcoind](https://github.com/nicholasgasior/qubitcoin).

Replaces the need for a separate Electrs/Esplora instance by running the indexer in-process as a qubitcoind secondary indexer module. All block data is indexed into an append-only key-value store and queried via view functions that return Esplora-compatible JSON.

## Overview

This crate compiles to `wasm32-unknown-unknown` and implements the metashrew indexer ABI:
- **`_start()`** — called for each new block, indexes transactions, UTXOs, addresses
- **`view_*`** — read-only query functions matching the Esplora REST API

## Supported Endpoints

| View Function | Esplora Endpoint | Description |
|---|---|---|
| `view_tx` | `GET /tx/:txid` | Full transaction details |
| `view_tx_hex` | `GET /tx/:txid/hex` | Hex-encoded raw transaction |
| `view_tx_raw` | `GET /tx/:txid/raw` | Raw transaction bytes |
| `view_tx_status` | `GET /tx/:txid/status` | Confirmation status |
| `view_tx_outspend` | `GET /tx/:txid/outspend/:vout` | Output spending status |
| `view_block` | `GET /block/:hash` | Block metadata |
| `view_block_status` | `GET /block/:hash/status` | Block chain status |
| `view_block_txids` | `GET /block/:hash/txids` | Transaction IDs in block |
| `view_block_header` | `GET /block/:hash/header` | Block header hex |
| `view_block_height` | `GET /block-height/:height` | Block hash at height |
| `view_tip_height` | `GET /blocks/tip/height` | Current chain tip height |
| `view_tip_hash` | `GET /blocks/tip/hash` | Current chain tip hash |
| `view_utxos_by_scripthash` | `GET /scripthash/:hash/utxo` | UTXOs by script hash |

## Building

```bash
# Native (for testing)
cargo build

# WASM (for deployment)
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

## Usage with qubitcoind

```bash
# Install the indexer
qubitcoin-cli installindexer esplora target/wasm32-unknown-unknown/release/esplorashrew.wasm

# Start qubitcoind with the indexer
qubitcoind -loadindexer=esplora:~/.local/qubitcoin/indexers/esplora

# Query via RPC
qubitcoin-cli secondaryheight esplora
qubitcoin-cli secondaryview esplora view_tip_height ""
qubitcoin-cli secondaryview esplora view_block_height "$(printf '%s' '100' | xxd -p)"
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
