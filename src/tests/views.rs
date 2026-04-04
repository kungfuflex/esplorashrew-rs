//! Tests for view functions — index blocks, then call views and verify JSON.

use crate::block::{self, to_hex_rev};
use crate::host;
use crate::tests::helpers;

/// Helper: call a view function by setting input and invoking the fn.
/// Returns the raw bytes from the ArrayBuffer pointer.
fn call_view(view_fn: fn() -> *const u8, input: &[u8]) -> Vec<u8> {
    // Prepend 4-byte height prefix (metashrew ABI: [height_le32 ++ payload])
    let mut prefixed = vec![0u8; 4]; // height = 0
    prefixed.extend_from_slice(input);
    host::set_input(prefixed);
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

// ---- utxosbyscripthash ----

#[test]
fn test_utxosbyscripthash_coinbase() {
    helpers::clear();

    // Index a block with a coinbase paying to OP_TRUE (0x51)
    let block_data = helpers::build_test_block(0);
    crate::index_block(0, &block_data);
    let parsed = block::parse_block(&block_data);

    // Compute the script hash for OP_TRUE
    let script_pubkey = &[0x51u8];
    let sh = block::script_hash(script_pubkey);

    // The view expects the script hash as reversed hex (display order)
    let sh_hex = to_hex_rev(&sh);

    // Query UTXOs
    let result = call_view_json(crate::views::utxosbyscripthash, &sh_hex);

    // Should be an array with exactly 1 UTXO (the coinbase output)
    let utxos = result.as_array().expect("expected JSON array");
    assert_eq!(utxos.len(), 1, "expected 1 UTXO, got {}: {:?}", utxos.len(), result);
    assert_eq!(utxos[0]["value"], 50_0000_0000u64);
    assert_eq!(utxos[0]["vout"], 0);

    let expected_txid = to_hex_rev(&parsed.transactions[0].txid);
    assert_eq!(utxos[0]["txid"], expected_txid);
    assert_eq!(utxos[0]["status"]["confirmed"], true);
    assert_eq!(utxos[0]["status"]["block_height"], 0);
}

#[test]
fn test_utxosbyscripthash_spent() {
    helpers::clear();

    // Block 0: coinbase paying to OP_TRUE
    let block0 = helpers::build_test_block(0);
    crate::index_block(0, &block0);
    let parsed0 = block::parse_block(&block0);
    let coinbase_txid = parsed0.transactions[0].txid;

    // Block 1: spend the coinbase, create new output to OP_TRUE
    let block1 = helpers::build_test_block_with_spend(
        1,
        &parsed0.header.hash,
        &coinbase_txid,
        0,
        49_0000_0000,
        &[0x51], // OP_TRUE output
    );
    crate::index_block(1, &block1);
    let parsed1 = block::parse_block(&block1);

    // Query UTXOs for OP_TRUE script hash
    let sh = block::script_hash(&[0x51u8]);
    let sh_hex = to_hex_rev(&sh);
    let result = call_view_json(crate::views::utxosbyscripthash, &sh_hex);

    let utxos = result.as_array().expect("expected JSON array");
    // The coinbase from block0 is spent. Block1 creates:
    // - coinbase output (to OP_TRUE)
    // - spending tx output (to OP_TRUE)
    // So we should have 2 unspent UTXOs (block1 coinbase + block1 spend tx)
    assert!(utxos.len() >= 2, "expected at least 2 UTXOs, got {}: {:?}", utxos.len(), result);

    // None of them should be the spent coinbase from block 0
    let coinbase0_txid_hex = to_hex_rev(&coinbase_txid);
    for utxo in utxos {
        assert_ne!(utxo["txid"], coinbase0_txid_hex,
            "spent coinbase from block 0 should not appear in UTXO set");
    }
}

#[test]
fn test_utxosbyscripthash_empty() {
    helpers::clear();

    // Index a block but query a different script hash
    let block = helpers::build_test_block(0);
    crate::index_block(0, &block);

    // Use a P2WPKH-like script that won't match the OP_TRUE coinbase
    let sh = block::script_hash(&[0x00, 0x14, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                                    0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12,
                                    0x13, 0x14]);
    let sh_hex = to_hex_rev(&sh);
    let result = call_view_json(crate::views::utxosbyscripthash, &sh_hex);

    let utxos = result.as_array().expect("expected JSON array");
    assert_eq!(utxos.len(), 0, "expected 0 UTXOs for unknown script");
}

#[test]
fn test_utxosbyscripthash_via_hex_encoded_input() {
    // Simulates the qubitcoind secondaryview path:
    // 1. Address → scriptPubKey → SHA256 → script_hash
    // 2. to_hex_rev(script_hash) → reversed hex string
    // 3. hex::encode(reversed_hex_string.as_bytes()) → hex-encoded ASCII
    // 4. qubitcoind hex-decodes → raw ASCII bytes = the reversed hex string
    // 5. View function receives the reversed hex string via load_input()

    helpers::clear();

    let block_data = helpers::build_test_block(0);
    crate::index_block(0, &block_data);

    // Compute script hash for OP_TRUE (same as coinbase)
    let script_pubkey = &[0x51u8];
    let sh = block::script_hash(script_pubkey);
    let sh_hex = to_hex_rev(&sh);

    // This is what the CLI's translate_esplora_for_qubitcoin does:
    // hex::encode(sh_hex.as_bytes()) — double-hex-encoding the string
    let hex_encoded = crate::block::to_hex(sh_hex.as_bytes());

    // Simulate qubitcoind's secondaryview: it hex-decodes the input
    let decoded_bytes = (0..hex_encoded.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_encoded[i..i+2], 16).unwrap())
        .collect::<Vec<u8>>();
    let decoded_string = String::from_utf8(decoded_bytes).unwrap();

    // This should equal our original sh_hex
    assert_eq!(decoded_string, sh_hex,
        "hex roundtrip should preserve the script hash string");

    // And querying with it should work
    let result = call_view_json(crate::views::utxosbyscripthash, &decoded_string);
    let utxos = result.as_array().expect("expected JSON array");
    assert_eq!(utxos.len(), 1, "expected 1 UTXO via hex-encoded path");
}

#[test]
fn test_utxosbyscripthash_p2tr_output() {
    helpers::clear();

    // Simulate a coinbase with P2TR output (like qubitcoind generatetoaddress)
    // P2TR scriptPubKey: OP_1 (0x51) PUSH32 (0x20) <32-byte-x-only-pubkey>
    let fake_pubkey = [0xabu8; 32];
    let mut p2tr_script = Vec::with_capacity(34);
    p2tr_script.push(0x51); // OP_1
    p2tr_script.push(0x20); // PUSH 32 bytes
    p2tr_script.extend_from_slice(&fake_pubkey);

    // Build a coinbase tx paying to this P2TR script
    let coinbase = helpers::build_spending_tx(
        &[0u8; 32],  // null prev txid (coinbase)
        0xFFFFFFFF,
        50_0000_0000,
        &p2tr_script,
    );

    // Wait — build_spending_tx doesn't set coinbase correctly.
    // Use build_coinbase_tx but with a custom script. Let me build manually.
    let mut tx = Vec::new();
    tx.extend_from_slice(&1i32.to_le_bytes()); // version
    tx.push(1u8); // 1 input
    tx.extend_from_slice(&[0u8; 32]); // null txid
    tx.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // coinbase vout
    // scriptsig: BIP34 height
    let height_bytes = 0u32.to_le_bytes();
    tx.push(5u8); // scriptsig len
    tx.push(4u8); // push 4 bytes
    tx.extend_from_slice(&height_bytes);
    tx.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // sequence
    tx.push(1u8); // 1 output
    tx.extend_from_slice(&50_0000_0000u64.to_le_bytes()); // value
    // P2TR scriptPubKey
    tx.push(p2tr_script.len() as u8); // scriptPubKey length
    tx.extend_from_slice(&p2tr_script);
    tx.extend_from_slice(&0u32.to_le_bytes()); // locktime

    let block = helpers::build_raw_block(
        0x20000000,
        &[0u8; 32],
        1231006505,
        0x1d00ffff,
        0,
        &[tx],
    );
    crate::index_block(0, &block);

    // Compute script hash for the P2TR scriptPubKey
    let sh = block::script_hash(&p2tr_script);
    let sh_hex = to_hex_rev(&sh);
    println!("P2TR script: {}", block::to_hex(&p2tr_script));
    println!("Script hash (raw): {}", block::to_hex(&sh));
    println!("Script hash (reversed hex for query): {}", sh_hex);

    let result = call_view_json(crate::views::utxosbyscripthash, &sh_hex);
    let utxos = result.as_array().expect("expected JSON array");
    assert_eq!(utxos.len(), 1, "expected 1 UTXO for P2TR address, got: {:?}", result);
    assert_eq!(utxos[0]["value"], 50_0000_0000u64);
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
