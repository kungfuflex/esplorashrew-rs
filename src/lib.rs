//! esplorashrew — Esplora-compatible indexer for qubitcoind's WASM runtime.
//!
//! This crate compiles to `wasm32-unknown-unknown` and implements the metashrew
//! indexer ABI. When loaded as a secondary indexer in qubitcoind, it indexes
//! blocks into an Esplora-compatible key-value schema and exposes view functions
//! that return the same JSON responses as the Blockstream Esplora REST API.
//!
//! # Indexed Data
//!
//! - Transactions: metadata, raw bytes, spending records
//! - Blocks: metadata, height-to-hash mapping, transaction lists
//! - Addresses/ScriptHashes: transaction history, UTXOs
//!
//! # View Functions (Esplora API)
//!
//! | View Function           | Esplora Endpoint              |
//! |------------------------|-------------------------------|
//! | `view_tx`              | `GET /tx/:txid`               |
//! | `view_tx_hex`          | `GET /tx/:txid/hex`           |
//! | `view_tx_raw`          | `GET /tx/:txid/raw`           |
//! | `view_tx_status`       | `GET /tx/:txid/status`        |
//! | `view_tx_outspend`     | `GET /tx/:txid/outspend/:vout`|
//! | `view_block`           | `GET /block/:hash`            |
//! | `view_block_status`    | `GET /block/:hash/status`     |
//! | `view_block_txids`     | `GET /block/:hash/txids`      |
//! | `view_block_header`    | `GET /block/:hash/header`     |
//! | `view_block_height`    | `GET /block-height/:height`   |
//! | `view_tip_height`      | `GET /blocks/tip/height`      |
//! | `view_tip_hash`        | `GET /blocks/tip/hash`        |
//! | `view_utxos_by_scripthash` | `GET /scripthash/:hash/utxo` |

mod block;
mod host;
mod indexer;
mod keys;
mod types;
mod views;

mod proto {
    include!(concat!(env!("OUT_DIR"), "/metashrew.rs"));
}

use prost::Message;

/// Entry point called by the qubitcoind WASM runtime for each new block.
///
/// Input format: `[height_le32 (4 bytes)][raw_block_data]`
#[no_mangle]
pub extern "C" fn _start() {
    let input = host::load_input();

    if input.len() < 4 {
        host::log("esplorashrew: input too short");
        // Still need to flush (empty) to signal completion.
        flush_empty();
        return;
    }

    // Parse height from first 4 bytes.
    let height = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    let block_data = &input[4..];

    // Parse the block.
    let parsed_block = block::parse_block(block_data);

    // Index the block.
    let pairs = indexer::index_block(height, &parsed_block, block_data.len() as u32);

    // Encode as KeyValueFlush protobuf and flush.
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

/// Flush an empty KeyValueFlush (signals completion with no state changes).
fn flush_empty() {
    let flush_msg = proto::KeyValueFlush { list: Vec::new() };
    let mut buf = Vec::with_capacity(flush_msg.encoded_len());
    flush_msg.encode(&mut buf).unwrap();
    host::flush(&buf);
}

// Re-export view functions so they're visible as WASM exports.
pub use views::*;
