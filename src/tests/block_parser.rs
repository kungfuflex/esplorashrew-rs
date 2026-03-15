//! Tests for the block/transaction parser.

use crate::block::*;
use crate::tests::helpers;

#[test]
fn test_parse_coinbase_tx() {
    let raw = helpers::build_coinbase_tx(100, 50_0000_0000);
    let (tx, consumed) = parse_transaction(&raw);
    assert_eq!(consumed, raw.len());
    assert_eq!(tx.version, 1);
    assert_eq!(tx.inputs.len(), 1);
    assert_eq!(tx.outputs.len(), 1);
    assert_eq!(tx.outputs[0].value, 50_0000_0000);
    assert_eq!(tx.outputs[0].script_pubkey, vec![0x51]); // OP_TRUE
    assert_eq!(tx.locktime, 0);
    // Coinbase input has null prevout.
    assert_eq!(tx.inputs[0].prev_txid, [0u8; 32]);
    assert_eq!(tx.inputs[0].prev_vout, 0xFFFFFFFF);
}

#[test]
fn test_parse_spending_tx() {
    let prev_txid = [0xABu8; 32];
    let script = vec![0x76, 0xa9, 0x14]; // OP_DUP OP_HASH160 OP_PUSH20
    let raw = helpers::build_spending_tx(&prev_txid, 0, 49_0000_0000, &script);
    let (tx, consumed) = parse_transaction(&raw);
    assert_eq!(consumed, raw.len());
    assert_eq!(tx.version, 2);
    assert_eq!(tx.inputs.len(), 1);
    assert_eq!(tx.inputs[0].prev_txid, prev_txid);
    assert_eq!(tx.inputs[0].prev_vout, 0);
    assert_eq!(tx.outputs.len(), 1);
    assert_eq!(tx.outputs[0].value, 49_0000_0000);
    assert_eq!(tx.outputs[0].script_pubkey, script);
}

#[test]
fn test_txid_is_hash256() {
    let raw = helpers::build_coinbase_tx(1, 50_0000_0000);
    let (tx, _) = parse_transaction(&raw);
    // txid should be double-SHA256 of the raw tx.
    let expected = hash256(&raw);
    assert_eq!(tx.txid, expected);
}

#[test]
fn test_different_heights_different_txids() {
    let raw1 = helpers::build_coinbase_tx(1, 50_0000_0000);
    let raw2 = helpers::build_coinbase_tx(2, 50_0000_0000);
    let (tx1, _) = parse_transaction(&raw1);
    let (tx2, _) = parse_transaction(&raw2);
    assert_ne!(tx1.txid, tx2.txid);
}

#[test]
fn test_parse_block_header() {
    let block_data = helpers::build_test_block(0);
    let header = parse_header(&block_data);
    assert_eq!(header.version, 0x20000000);
    assert_eq!(header.prev_block, [0u8; 32]);
    assert_eq!(header.time, 1231006505);
    assert_eq!(header.bits, 0x1d00ffff);
    assert_eq!(header.nonce, 0);
    // Hash should be non-zero.
    assert_ne!(header.hash, [0u8; 32]);
}

#[test]
fn test_parse_block_one_tx() {
    let block_data = helpers::build_test_block(0);
    let block = parse_block(&block_data);
    assert_eq!(block.transactions.len(), 1);
    assert_eq!(block.header.version, 0x20000000);
    assert_eq!(block.raw_header.len(), 80);
}

#[test]
fn test_parse_block_two_txs() {
    let prev_txid = [0x42u8; 32];
    let block_data = helpers::build_test_block_with_spend(
        1,
        &[0u8; 32],
        &prev_txid,
        0,
        49_0000_0000,
        &[0x51], // OP_TRUE
    );
    let block = parse_block(&block_data);
    assert_eq!(block.transactions.len(), 2);
    // First tx is coinbase.
    assert_eq!(block.transactions[0].inputs[0].prev_txid, [0u8; 32]);
    // Second tx spends prev_txid.
    assert_eq!(block.transactions[1].inputs[0].prev_txid, prev_txid);
}

#[test]
fn test_hash256() {
    // Known: SHA256(SHA256("")) = e3b0c44298fc... double-hashed
    let empty_hash = hash256(b"");
    assert_ne!(empty_hash, [0u8; 32]);
    // Deterministic.
    assert_eq!(empty_hash, hash256(b""));
}

#[test]
fn test_to_hex_and_rev() {
    let bytes = [0xAB, 0xCD, 0xEF];
    assert_eq!(to_hex(&bytes), "abcdef");
    assert_eq!(to_hex_rev(&bytes), "efcdab");
}

#[test]
fn test_script_hash() {
    let script = vec![0x51]; // OP_TRUE
    let sh = script_hash(&script);
    assert_ne!(sh, [0u8; 32]);
    // Deterministic.
    assert_eq!(sh, script_hash(&script));
    // Different script = different hash.
    assert_ne!(sh, script_hash(&[0x00]));
}

#[test]
fn test_compact_size_roundtrip() {
    for n in [0u64, 1, 252, 253, 0xFFFF, 0x10000, 0xFFFFFFFF] {
        let encoded = encode_compact_size(n);
        let (decoded, consumed) = read_compact_size(&encoded);
        assert_eq!(decoded, n);
        assert_eq!(consumed, encoded.len());
    }
}

#[test]
fn test_tx_weight_nonwitness() {
    let raw = helpers::build_coinbase_tx(0, 50_0000_0000);
    let (tx, _) = parse_transaction(&raw);
    // Non-witness: weight = size * 4
    assert_eq!(tx.weight, tx.size * 4);
}
