//! esplorashrew — Esplora-compatible indexer for qubitcoind's WASM runtime.
//!
//! This crate compiles to `wasm32-unknown-unknown` and implements the metashrew
//! indexer ABI. When loaded as a secondary indexer in qubitcoind, it indexes
//! blocks into an Esplora-compatible key-value schema and exposes view functions
//! that return the same JSON responses as the Blockstream Esplora REST API.
//!
//! # View Functions (Esplora API)
//!
//! | View Function        | Esplora Endpoint              |
//! |---------------------|-------------------------------|
//! | `tx`                | `GET /tx/:txid`               |
//! | `txhex`             | `GET /tx/:txid/hex`           |
//! | `txraw`             | `GET /tx/:txid/raw`           |
//! | `txstatus`          | `GET /tx/:txid/status`        |
//! | `txoutspend`        | `GET /tx/:txid/outspend/:vout`|
//! | `block`             | `GET /block/:hash`            |
//! | `blockstatus`       | `GET /block/:hash/status`     |
//! | `blocktxids`        | `GET /block/:hash/txids`      |
//! | `blockheader`       | `GET /block/:hash/header`     |
//! | `blockheight`       | `GET /block-height/:height`   |
//! | `tipheight`         | `GET /blocks/tip/height`      |
//! | `tiphash`           | `GET /blocks/tip/hash`        |
//! | `utxosbyscripthash` | `GET /scripthash/:hash/utxo`  |

pub mod block;
pub mod host;
pub mod indexer;
pub mod keys;
pub mod types;
pub mod views;

#[cfg(test)]
mod tests;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/metashrew.rs"));
}

use prost::Message;

/// Index a block: parse it, generate KV pairs, flush to storage.
///
/// This is the core indexing function, callable both from `_start()` (WASM)
/// and from tests (native).
pub fn index_block(height: u32, block_data: &[u8]) {
    let parsed_block = block::parse_block(block_data);
    let pairs = indexer::index_block(height, &parsed_block, block_data.len() as u32);

    let mut list = Vec::with_capacity(pairs.len() * 2);
    for (key, value) in &pairs {
        list.push(key.clone());
        list.push(value.clone());
    }

    let flush_msg = proto::KeyValueFlush { list };
    let mut buf = Vec::with_capacity(flush_msg.encoded_len());
    flush_msg.encode(&mut buf).unwrap();

    host::flush(&buf);
}

/// Entry point called by the qubitcoind WASM runtime for each new block.
///
/// Input format: `[height_le32 (4 bytes)][raw_block_data]`
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() {
    let input = host::load_input();

    if input.len() < 4 {
        host::log("esplorashrew: input too short");
        flush_empty();
        return;
    }

    let height = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    let block_data = &input[4..];

    index_block(height, block_data);
}

/// Flush an empty KeyValueFlush (signals completion with no state changes).
fn flush_empty() {
    let flush_msg = proto::KeyValueFlush { list: Vec::new() };
    let mut buf = Vec::with_capacity(flush_msg.encoded_len());
    flush_msg.encode(&mut buf).unwrap();
    host::flush(&buf);
}

// Re-export view functions so they're visible as WASM exports.
pub use views::*;
