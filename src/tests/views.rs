//! Tests for view functions — index blocks, then call views and verify JSON.

use crate::block::{self, to_hex_rev};
use crate::host;
use crate::tests::helpers;

/// Helper: call a view function by setting input and invoking the fn.
/// Returns the raw bytes from the ArrayBuffer pointer.
fn call_view(view_fn: fn() -> *const u8, input: &[u8]) -> Vec<u8> {
    host::set_input(input.to_vec());
    let ptr = view_fn();
    // Read ArrayBuffer: length at (ptr-4), data at ptr.
    unsafe {
        let len_ptr = ptr.sub(4);
        let len = u32::from_le_bytes([
            *len_ptr,
            *len_ptr.add(1),
            *len_ptr.add(2),
            *len_ptr.add(3),
        ]) as usize;
        std::slice::from_raw_parts(ptr, len).to_vec()
    }
}

/// Helper: call a view and parse result as JSON.
fn call_view_json(view_fn: fn() -> *const u8, input: &str) -> serde_json::Value {
    let bytes = call_view(view_fn, input.as_bytes());
    serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        serde_json::Value::String(String::from_utf8_lossy(&bytes).to_string())
    })
}

/// Helper: call a view and get result as string.
fn call_view_string(view_fn: fn() -> *const u8, input: &str) -> String {
    let bytes = call_view(view_fn, input.as_bytes());
    String::from_utf8_lossy(&bytes).to_string()
}

// ---- tipheight / tiphash ----

#[test]
fn test_tipheight_empty() {
    helpers::clear();
    let result = call_view_string(crate::views::tipheight, "");
    assert_eq!(result, "0");
}

#[test]
fn test_tipheight_after_indexing() {
    helpers::clear();
    let block = helpers::build_test_block(0);
    crate::index_block(0, &block);
    let result = call_view_string(crate::views::tipheight, "");
    assert_eq!(result, "0");

    // Index another block.
    let parsed = block::parse_block(&block);
    let coinbase = helpers::build_coinbase_tx(1, 50_0000_0000);
    let block1 = helpers::build_raw_block(
        0x20000000,
        &parsed.header.hash,
        1231006506,
        0x1d00ffff,
        0,
        &[coinbase],
    );
    crate::index_block(1, &block1);
    let result = call_view_string(crate::views::tipheight, "");
    assert_eq!(result, "1");
}

#[test]
fn test_tiphash_after_indexing() {
    helpers::clear();
    let block = helpers::build_test_block(0);
    crate::index_block(0, &block);

    let parsed = block::parse_block(&block);
    let expected_hash = to_hex_rev(&parsed.header.hash);

    let result = call_view_string(crate::views::tiphash, "");
    assert_eq!(result, expected_hash);
}

// ---- blockheight ----

#[test]
fn test_blockheight() {
    helpers::clear();
    let block = helpers::build_test_block(0);
    crate::index_block(0, &block);

    let parsed = block::parse_block(&block);
    let expected_hash = to_hex_rev(&parsed.header.hash);

    let result = call_view_string(crate::views::blockheight, "0");
    assert_eq!(result, expected_hash);
}

#[test]
fn test_blockheight_not_found() {
    helpers::clear();
    let result = call_view_string(crate::views::blockheight, "999");
    assert_eq!(result, "block not found");
}

// ---- block ----

#[test]
fn test_block_view() {
    helpers::clear();
    let block_data = helpers::build_test_block(0);
    crate::index_block(0, &block_data);

    let parsed = block::parse_block(&block_data);
    let hash_hex = to_hex_rev(&parsed.header.hash);

    let result = call_view_json(crate::views::block, &hash_hex);
    assert_eq!(result["height"], 0);
    assert_eq!(result["tx_count"], 1);
    assert_eq!(result["id"], hash_hex);
}

#[test]
fn test_block_view_not_found() {
    helpers::clear();
    let fake_hash = "00".repeat(32);
    let result = call_view_json(crate::views::block, &fake_hash);
    assert!(result["error"].is_string());
}

// ---- blockstatus ----

#[test]
fn test_blockstatus() {
    helpers::clear();
    let block_data = helpers::build_test_block(0);
    crate::index_block(0, &block_data);

    let parsed = block::parse_block(&block_data);
    let hash_hex = to_hex_rev(&parsed.header.hash);

    let result = call_view_json(crate::views::blockstatus, &hash_hex);
    assert_eq!(result["in_best_chain"], true);
    assert_eq!(result["height"], 0);
}

// ---- blocktxids ----

#[test]
fn test_blocktxids() {
    helpers::clear();
    let block_data = helpers::build_test_block(0);
    crate::index_block(0, &block_data);

    let parsed = block::parse_block(&block_data);
    let hash_hex = to_hex_rev(&parsed.header.hash);
    let expected_txid = to_hex_rev(&parsed.transactions[0].txid);

    let result = call_view_json(crate::views::blocktxids, &hash_hex);
    let txids = result.as_array().unwrap();
    assert_eq!(txids.len(), 1);
    assert_eq!(txids[0].as_str().unwrap(), expected_txid);
}

// ---- tx ----

#[test]
fn test_tx_view() {
    helpers::clear();
    let block_data = helpers::build_test_block(10);
    crate::index_block(10, &block_data);

    let parsed = block::parse_block(&block_data);
    let txid_hex = to_hex_rev(&parsed.transactions[0].txid);

    let result = call_view_json(crate::views::tx, &txid_hex);
    assert_eq!(result["txid"], txid_hex);
    assert_eq!(result["version"], 1);
    assert_eq!(result["status"]["confirmed"], true);
    assert_eq!(result["status"]["block_height"], 10);

    let vin = result["vin"].as_array().unwrap();
    assert_eq!(vin.len(), 1);

    let vout = result["vout"].as_array().unwrap();
    assert_eq!(vout.len(), 1);
    assert_eq!(vout[0]["value"], 50_0000_0000u64);
}

#[test]
fn test_tx_not_found() {
    helpers::clear();
    let fake_txid = "ff".repeat(32);
    let result = call_view_json(crate::views::tx, &fake_txid);
    assert!(result["error"].is_string());
}

// ---- txhex ----

#[test]
fn test_txhex() {
    helpers::clear();
    let block_data = helpers::build_test_block(0);
    crate::index_block(0, &block_data);

    let parsed = block::parse_block(&block_data);
    let txid_hex = to_hex_rev(&parsed.transactions[0].txid);

    let result = call_view_string(crate::views::txhex, &txid_hex);
    // Should be valid hex.
    assert!(result.len() > 0);
    assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
}

// ---- txstatus ----

#[test]
fn test_txstatus_confirmed() {
    helpers::clear();
    let block_data = helpers::build_test_block(7);
    crate::index_block(7, &block_data);

    let parsed = block::parse_block(&block_data);
    let txid_hex = to_hex_rev(&parsed.transactions[0].txid);

    let result = call_view_json(crate::views::txstatus, &txid_hex);
    assert_eq!(result["confirmed"], true);
    assert_eq!(result["block_height"], 7);
}

#[test]
fn test_txstatus_not_found() {
    helpers::clear();
    let fake_txid = "aa".repeat(32);
    let result = call_view_json(crate::views::txstatus, &fake_txid);
    assert_eq!(result["confirmed"], false);
}

// ---- txoutspend ----

#[test]
fn test_txoutspend_unspent() {
    helpers::clear();
    let block_data = helpers::build_test_block(0);
    crate::index_block(0, &block_data);

    let parsed = block::parse_block(&block_data);
    let txid_hex = to_hex_rev(&parsed.transactions[0].txid);

    let input = serde_json::json!({"txid": txid_hex, "vout": 0}).to_string();
    let result = call_view_json(crate::views::txoutspend, &input);
    assert_eq!(result["spent"], false);
}

#[test]
fn test_txoutspend_spent() {
    helpers::clear();

    // Block 0: coinbase.
    let block0 = helpers::build_test_block(0);
    crate::index_block(0, &block0);
    let parsed0 = block::parse_block(&block0);
    let coinbase_txid = parsed0.transactions[0].txid;

    // Block 1: spends coinbase output.
    let block1 = helpers::build_test_block_with_spend(
        1,
        &parsed0.header.hash,
        &coinbase_txid,
        0,
        49_0000_0000,
        &[0x51],
    );
    crate::index_block(1, &block1);

    let txid_hex = to_hex_rev(&coinbase_txid);
    let input = serde_json::json!({"txid": txid_hex, "vout": 0}).to_string();
    let result = call_view_json(crate::views::txoutspend, &input);
    assert_eq!(result["spent"], true);
    assert!(result["txid"].is_string());
}

// ---- end-to-end: index multiple blocks, query everything ----

#[test]
fn test_end_to_end_two_blocks() {
    helpers::clear();

    // Block 0.
    let block0 = helpers::build_test_block(0);
    crate::index_block(0, &block0);
    let parsed0 = block::parse_block(&block0);

    // Block 1.
    let coinbase1 = helpers::build_coinbase_tx(1, 50_0000_0000);
    let block1 = helpers::build_raw_block(
        0x20000000,
        &parsed0.header.hash,
        1231006506,
        0x1d00ffff,
        1,
        &[coinbase1],
    );
    crate::index_block(1, &block1);
    let parsed1 = block::parse_block(&block1);

    // Tip should be 1.
    assert_eq!(call_view_string(crate::views::tipheight, ""), "1");

    // Tip hash should match block 1.
    let tip_hash = call_view_string(crate::views::tiphash, "");
    assert_eq!(tip_hash, to_hex_rev(&parsed1.header.hash));

    // blockheight 0 should return block 0 hash.
    let hash0 = call_view_string(crate::views::blockheight, "0");
    assert_eq!(hash0, to_hex_rev(&parsed0.header.hash));

    // blockheight 1 should return block 1 hash.
    let hash1 = call_view_string(crate::views::blockheight, "1");
    assert_eq!(hash1, to_hex_rev(&parsed1.header.hash));

    // Both blocks queryable.
    let b0 = call_view_json(crate::views::block, &hash0);
    assert_eq!(b0["height"], 0);
    let b1 = call_view_json(crate::views::block, &hash1);
    assert_eq!(b1["height"], 1);

    // Block 0 status should show block 1 as next_best.
    let status0 = call_view_json(crate::views::blockstatus, &hash0);
    assert_eq!(status0["next_best"], hash1);

    // All txids queryable.
    let txid0 = to_hex_rev(&parsed0.transactions[0].txid);
    let tx0 = call_view_json(crate::views::tx, &txid0);
    assert_eq!(tx0["status"]["block_height"], 0);

    let txid1 = to_hex_rev(&parsed1.transactions[0].txid);
    let tx1 = call_view_json(crate::views::tx, &txid1);
    assert_eq!(tx1["status"]["block_height"], 1);
}
