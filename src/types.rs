//! Serializable types for the Esplora index.

use serde::{Deserialize, Serialize};

/// Metadata stored for each transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxMeta {
    /// Block height where the transaction was confirmed.
    pub block_height: u32,
    /// Block hash (hex).
    pub block_hash: String,
    /// Block timestamp.
    pub block_time: u32,
    /// Transaction fee in satoshis.
    pub fee: u64,
    /// Transaction size in bytes.
    pub size: u32,
    /// Transaction weight units.
    pub weight: u32,
    /// Transaction version.
    pub version: i32,
    /// Locktime.
    pub locktime: u32,
    /// Index within the block.
    pub tx_index: u32,
    /// Number of inputs.
    pub vin_count: u32,
    /// Number of outputs.
    pub vout_count: u32,
}

/// Metadata stored for each block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockMeta {
    /// Block hash (hex).
    pub id: String,
    /// Block height.
    pub height: u32,
    /// Block version.
    pub version: i32,
    /// Block timestamp.
    pub timestamp: u32,
    /// Merkle root (hex).
    pub merkle_root: String,
    /// Number of transactions.
    pub tx_count: u32,
    /// Block size in bytes.
    pub size: u32,
    /// Block weight.
    pub weight: u32,
    /// Bits (compact target).
    pub bits: u32,
    /// Nonce.
    pub nonce: u32,
    /// Previous block hash (hex).
    pub previousblockhash: String,
}

/// Address statistics (cumulative).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddressStats {
    pub tx_count: u32,
    pub funded_txo_count: u32,
    pub funded_txo_sum: u64,
    pub spent_txo_count: u32,
    pub spent_txo_sum: u64,
}

/// UTXO entry stored in the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoEntry {
    pub txid: String,
    pub vout: u32,
    pub value: u64,
    pub block_height: u32,
    pub block_hash: String,
    pub block_time: u32,
}

/// Spending info for a transaction output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendInfo {
    pub spent: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vin: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<TxStatus>,
}

/// Transaction confirmation status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxStatus {
    pub confirmed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_time: Option<u32>,
}

/// Block status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockStatus {
    pub in_best_chain: bool,
    pub height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_best: Option<String>,
}
