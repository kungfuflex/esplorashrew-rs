//! Test helpers for building synthetic Bitcoin blocks.

use crate::block::{encode_compact_size, hash256};
use crate::host;

/// Clear all indexer state between tests.
pub fn clear() {
    host::clear();
}

/// Build a minimal coinbase transaction.
///
/// Returns raw serialized transaction bytes.
pub fn build_coinbase_tx(height: u32, value_sats: u64) -> Vec<u8> {
    let mut tx = Vec::new();

    // version (i32 LE)
    tx.extend_from_slice(&1i32.to_le_bytes());

    // input count = 1
    tx.push(1u8);

    // coinbase input: prev_txid = 0x00*32, prev_vout = 0xFFFFFFFF
    tx.extend_from_slice(&[0u8; 32]); // null txid
    tx.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // vout

    // scriptsig: push height as BIP34
    let height_bytes = height.to_le_bytes();
    let sig_len = 4 + 1; // 1-byte push + 4 bytes height
    tx.extend_from_slice(&encode_compact_size(sig_len as u64));
    tx.push(4u8); // push 4 bytes
    tx.extend_from_slice(&height_bytes);

    // sequence
    tx.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());

    // output count = 1
    tx.push(1u8);

    // output value
    tx.extend_from_slice(&value_sats.to_le_bytes());

    // scriptPubKey: OP_TRUE (0x51) — simplest valid output
    tx.extend_from_slice(&encode_compact_size(1));
    tx.push(0x51); // OP_TRUE

    // locktime
    tx.extend_from_slice(&0u32.to_le_bytes());

    tx
}

/// Build a simple P2PKH-like spending transaction.
///
/// Spends from `prev_txid:prev_vout` and creates an output paying `value_sats`
/// to the given `script_pubkey`.
pub fn build_spending_tx(
    prev_txid: &[u8; 32],
    prev_vout: u32,
    value_sats: u64,
    script_pubkey: &[u8],
) -> Vec<u8> {
    let mut tx = Vec::new();

    // version
    tx.extend_from_slice(&2i32.to_le_bytes());

    // input count = 1
    tx.push(1u8);

    // input
    tx.extend_from_slice(prev_txid);
    tx.extend_from_slice(&prev_vout.to_le_bytes());
    // empty scriptsig
    tx.push(0u8);
    // sequence
    tx.extend_from_slice(&0xFFFFFFFEu32.to_le_bytes());

    // output count = 1
    tx.push(1u8);

    // output
    tx.extend_from_slice(&value_sats.to_le_bytes());
    tx.extend_from_slice(&encode_compact_size(script_pubkey.len() as u64));
    tx.extend_from_slice(script_pubkey);

    // locktime
    tx.extend_from_slice(&0u32.to_le_bytes());

    tx
}

/// Build a raw block from a header and a list of raw transaction bytes.
pub fn build_raw_block(
    version: i32,
    prev_block: &[u8; 32],
    time: u32,
    bits: u32,
    nonce: u32,
    txs: &[Vec<u8>],
) -> Vec<u8> {
    // Compute merkle root from transaction hashes.
    let tx_hashes: Vec<[u8; 32]> = txs.iter().map(|tx| hash256(tx)).collect();
    let merkle_root = compute_merkle_root(&tx_hashes);

    let mut block = Vec::new();

    // Header (80 bytes).
    block.extend_from_slice(&version.to_le_bytes());
    block.extend_from_slice(prev_block);
    block.extend_from_slice(&merkle_root);
    block.extend_from_slice(&time.to_le_bytes());
    block.extend_from_slice(&bits.to_le_bytes());
    block.extend_from_slice(&nonce.to_le_bytes());

    // Transaction count.
    block.extend_from_slice(&encode_compact_size(txs.len() as u64));

    // Transactions.
    for tx in txs {
        block.extend_from_slice(tx);
    }

    block
}

/// Compute merkle root from a list of transaction hashes.
fn compute_merkle_root(hashes: &[[u8; 32]]) -> [u8; 32] {
    if hashes.is_empty() {
        return [0u8; 32];
    }
    if hashes.len() == 1 {
        return hashes[0];
    }

    let mut level = hashes.to_vec();
    while level.len() > 1 {
        let mut next = Vec::new();
        for i in (0..level.len()).step_by(2) {
            let left = &level[i];
            let right = if i + 1 < level.len() {
                &level[i + 1]
            } else {
                &level[i] // duplicate last if odd
            };
            let mut combined = Vec::with_capacity(64);
            combined.extend_from_slice(left);
            combined.extend_from_slice(right);
            next.push(hash256(&combined));
        }
        level = next;
    }
    level[0]
}

/// Build a simple test block at the given height with one coinbase tx.
pub fn build_test_block(height: u32) -> Vec<u8> {
    let coinbase = build_coinbase_tx(height, 50_0000_0000);
    build_raw_block(
        0x20000000,       // version
        &[0u8; 32],       // prev_block (genesis)
        1231006505 + height, // time
        0x1d00ffff,       // bits
        0,                // nonce
        &[coinbase],
    )
}

/// Build a test block with a coinbase + a spending transaction.
pub fn build_test_block_with_spend(
    height: u32,
    prev_block: &[u8; 32],
    prev_txid: &[u8; 32],
    prev_vout: u32,
    spend_value: u64,
    spend_script: &[u8],
) -> Vec<u8> {
    let coinbase = build_coinbase_tx(height, 50_0000_0000);
    let spend_tx = build_spending_tx(prev_txid, prev_vout, spend_value, spend_script);
    build_raw_block(
        0x20000000,
        prev_block,
        1231006505 + height,
        0x1d00ffff,
        0,
        &[coinbase, spend_tx],
    )
}
