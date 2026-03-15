//! Block indexing logic.
//!
//! Processes each block and produces key-value pairs for the Esplora index.

use crate::block::{self, Block, Transaction};
use crate::keys;
use crate::types::*;

/// Process a block and return the key-value pairs to flush.
pub fn index_block(
    height: u32,
    block: &Block,
    block_raw_size: u32,
) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut pairs = Vec::new();

    let block_hash = block.header.hash;
    let block_hash_hex = block::to_hex_rev(&block_hash);
    let prev_hash_hex = block::to_hex_rev(&block.header.prev_block);
    let merkle_hex = block::to_hex_rev(&block.header.merkle_root);

    // Compute total block weight (sum of tx weights).
    let total_weight: u32 = block.transactions.iter().map(|tx| tx.weight).sum();

    // Store block metadata.
    let block_meta = BlockMeta {
        id: block_hash_hex.clone(),
        height,
        version: block.header.version,
        timestamp: block.header.time,
        merkle_root: merkle_hex,
        tx_count: block.transactions.len() as u32,
        size: block_raw_size,
        weight: total_weight,
        bits: block.header.bits,
        nonce: block.header.nonce,
        previousblockhash: prev_hash_hex,
    };
    let meta_bytes = serde_json::to_vec(&block_meta).unwrap_or_default();
    pairs.push((keys::block_key(&block_hash), meta_bytes));

    // Store height -> hash mapping.
    pairs.push((keys::block_height_key(height), block_hash.to_vec()));

    // Store tx count for block.
    pairs.push((
        keys::block_tx_count_key(&block_hash),
        (block.transactions.len() as u32).to_le_bytes().to_vec(),
    ));

    // Update tip.
    pairs.push((keys::TIP_KEY.to_vec(), height.to_le_bytes().to_vec()));
    pairs.push((keys::TIP_HASH_KEY.to_vec(), block_hash.to_vec()));

    // Process each transaction.
    for (tx_index, tx) in block.transactions.iter().enumerate() {
        let txid = tx.txid;
        let txid_hex = block::to_hex_rev(&txid);

        // Store txid in block's tx list.
        pairs.push((
            keys::block_txid_key(&block_hash, tx_index as u32),
            txid.to_vec(),
        ));

        // Compute fee (sum of input values - sum of output values).
        // For coinbase, fee = 0. For non-coinbase, we'd need input values
        // from previous transactions. We store 0 for now and compute
        // fee lazily in views by looking up spent outputs.
        let fee = 0u64; // TODO: compute from inputs when available

        // Store transaction metadata.
        let tx_meta = TxMeta {
            block_height: height,
            block_hash: block_hash_hex.clone(),
            block_time: block.header.time,
            fee,
            size: tx.size,
            weight: tx.weight,
            version: tx.version,
            locktime: tx.locktime,
            tx_index: tx_index as u32,
            vin_count: tx.inputs.len() as u32,
            vout_count: tx.outputs.len() as u32,
        };
        let tx_meta_bytes = serde_json::to_vec(&tx_meta).unwrap_or_default();
        pairs.push((keys::tx_key(&txid), tx_meta_bytes));

        // Store raw transaction.
        pairs.push((keys::tx_raw_key(&txid), tx.raw.clone()));

        // Index inputs (spending records).
        let is_coinbase = tx_index == 0;
        if !is_coinbase {
            for (vin_idx, input) in tx.inputs.iter().enumerate() {
                // Record that this output is spent by this tx.
                pairs.push((
                    keys::spend_key(&input.prev_txid, input.prev_vout),
                    txid.to_vec(),
                ));

                // Remove UTXO for the spent output.
                // We need the scriptPubKey of the spent output to compute the
                // script hash. This would require looking up the previous tx.
                // For now, we handle this in the view layer.
            }
        }

        // Index outputs (UTXOs and address mappings).
        for (vout, output) in tx.outputs.iter().enumerate() {
            if output.script_pubkey.is_empty() {
                continue;
            }

            let sh = block::script_hash(&output.script_pubkey);

            // Store UTXO.
            let utxo = UtxoEntry {
                txid: txid_hex.clone(),
                vout: vout as u32,
                value: output.value,
                block_height: height,
                block_hash: block_hash_hex.clone(),
                block_time: block.header.time,
            };
            let utxo_bytes = serde_json::to_vec(&utxo).unwrap_or_default();
            pairs.push((
                keys::utxo_key(&sh, &txid, vout as u32),
                utxo_bytes,
            ));

            // Store address -> tx mapping.
            pairs.push((
                keys::address_tx_key(&sh, height, tx_index as u16),
                txid.to_vec(),
            ));
        }
    }

    pairs
}
