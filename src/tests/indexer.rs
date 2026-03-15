//! Tests for the block indexer — verifies KV pairs written to the cache.

use crate::block::{self, to_hex_rev};
use crate::host;
use crate::keys;
use crate::tests::helpers;
use crate::types::*;

#[test]
fn test_index_single_block() {
    helpers::clear();
    let block_data = helpers::build_test_block(0);
    crate::index_block(0, &block_data);

    // Verify tip height was stored.
    let tip = host::cache_get(keys::TIP_KEY).unwrap();
    assert_eq!(u32::from_le_bytes([tip[0], tip[1], tip[2], tip[3]]), 0);

    // Verify tip hash was stored.
    let tip_hash = host::cache_get(keys::TIP_HASH_KEY).unwrap();
    assert_eq!(tip_hash.len(), 32);

    // Verify block metadata.
    let parsed = block::parse_block(&block_data);
    let block_hash = parsed.header.hash;
    let meta_bytes = host::cache_get(&keys::block_key(&block_hash)).unwrap();
    let meta: BlockMeta = serde_json::from_slice(&meta_bytes).unwrap();
    assert_eq!(meta.height, 0);
    assert_eq!(meta.tx_count, 1);
    assert_eq!(meta.version, 0x20000000);

    // Verify height -> hash mapping.
    let hash_at_0 = host::cache_get(&keys::block_height_key(0)).unwrap();
    assert_eq!(hash_at_0, block_hash.to_vec());
}

#[test]
fn test_index_tx_metadata() {
    helpers::clear();
    let block_data = helpers::build_test_block(42);
    crate::index_block(42, &block_data);

    let parsed = block::parse_block(&block_data);
    let txid = parsed.transactions[0].txid;

    // Verify tx metadata.
    let meta_bytes = host::cache_get(&keys::tx_key(&txid)).unwrap();
    let meta: TxMeta = serde_json::from_slice(&meta_bytes).unwrap();
    assert_eq!(meta.block_height, 42);
    assert_eq!(meta.tx_index, 0);
    assert_eq!(meta.vin_count, 1);
    assert_eq!(meta.vout_count, 1);
}

#[test]
fn test_index_tx_raw() {
    helpers::clear();
    let block_data = helpers::build_test_block(1);
    crate::index_block(1, &block_data);

    let parsed = block::parse_block(&block_data);
    let txid = parsed.transactions[0].txid;

    // Verify raw tx is stored.
    let raw = host::cache_get(&keys::tx_raw_key(&txid)).unwrap();
    assert!(!raw.is_empty());
    // Re-parse the stored raw — should get the same txid.
    let (reparsed, _) = block::parse_transaction(&raw);
    assert_eq!(reparsed.txid, txid);
}

#[test]
fn test_index_block_txids() {
    helpers::clear();
    let block_data = helpers::build_test_block(0);
    crate::index_block(0, &block_data);

    let parsed = block::parse_block(&block_data);
    let block_hash = parsed.header.hash;

    // Verify tx count.
    let count_bytes = host::cache_get(&keys::block_tx_count_key(&block_hash)).unwrap();
    let count = u32::from_le_bytes([count_bytes[0], count_bytes[1], count_bytes[2], count_bytes[3]]);
    assert_eq!(count, 1);

    // Verify txid at index 0.
    let txid_bytes = host::cache_get(&keys::block_txid_key(&block_hash, 0)).unwrap();
    assert_eq!(txid_bytes.len(), 32);
    assert_eq!(txid_bytes, parsed.transactions[0].txid.to_vec());
}

#[test]
fn test_index_spending_creates_spend_record() {
    helpers::clear();

    // First block with coinbase.
    let block0 = helpers::build_test_block(0);
    crate::index_block(0, &block0);

    let parsed0 = block::parse_block(&block0);
    let coinbase_txid = parsed0.transactions[0].txid;
    let block0_hash = parsed0.header.hash;

    // Second block spending the coinbase output.
    let block1 = helpers::build_test_block_with_spend(
        1,
        &block0_hash,
        &coinbase_txid,
        0,
        49_0000_0000,
        &[0x51], // OP_TRUE
    );
    crate::index_block(1, &block1);

    // Verify spend record exists.
    let spend_key = keys::spend_key(&coinbase_txid, 0);
    let spending_txid = host::cache_get(&spend_key).unwrap();
    assert_eq!(spending_txid.len(), 32);

    // The spending txid should be the second tx in block 1.
    let parsed1 = block::parse_block(&block1);
    assert_eq!(spending_txid, parsed1.transactions[1].txid.to_vec());
}

#[test]
fn test_index_multiple_blocks_updates_tip() {
    helpers::clear();

    let block0 = helpers::build_test_block(0);
    crate::index_block(0, &block0);

    let parsed0 = block::parse_block(&block0);
    let block0_hash = parsed0.header.hash;

    // Build block 1 with prev_block = block 0 hash.
    let coinbase1 = helpers::build_coinbase_tx(1, 50_0000_0000);
    let block1 = helpers::build_raw_block(
        0x20000000,
        &block0_hash,
        1231006506,
        0x1d00ffff,
        0,
        &[coinbase1],
    );
    crate::index_block(1, &block1);

    // Tip should be at height 1.
    let tip = host::cache_get(keys::TIP_KEY).unwrap();
    assert_eq!(u32::from_le_bytes([tip[0], tip[1], tip[2], tip[3]]), 1);

    // Both heights should have entries.
    assert!(host::cache_get(&keys::block_height_key(0)).is_some());
    assert!(host::cache_get(&keys::block_height_key(1)).is_some());
}

#[test]
fn test_index_utxo_created() {
    helpers::clear();
    let block_data = helpers::build_test_block(0);
    crate::index_block(0, &block_data);

    let parsed = block::parse_block(&block_data);
    let txid = parsed.transactions[0].txid;
    let script = &parsed.transactions[0].outputs[0].script_pubkey;
    let sh = block::script_hash(script);

    // UTXO should be stored.
    let utxo_bytes = host::cache_get(&keys::utxo_key(&sh, &txid, 0)).unwrap();
    let utxo: UtxoEntry = serde_json::from_slice(&utxo_bytes).unwrap();
    assert_eq!(utxo.value, 50_0000_0000);
    assert_eq!(utxo.vout, 0);
    assert_eq!(utxo.block_height, 0);
}

#[test]
fn test_index_address_tx_mapping() {
    helpers::clear();
    let block_data = helpers::build_test_block(5);
    crate::index_block(5, &block_data);

    let parsed = block::parse_block(&block_data);
    let txid = parsed.transactions[0].txid;
    let script = &parsed.transactions[0].outputs[0].script_pubkey;
    let sh = block::script_hash(script);

    // Address -> tx mapping should exist.
    let addr_tx = host::cache_get(&keys::address_tx_key(&sh, 5, 0)).unwrap();
    assert_eq!(addr_tx.len(), 32);
    assert_eq!(addr_tx, txid.to_vec());
}
