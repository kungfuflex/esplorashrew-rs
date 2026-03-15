//! Key schema for the Esplora index.
//!
//! All keys are prefixed with a single byte tag to namespace them.
//! This enables efficient prefix scanning and avoids collisions.

// --- Key prefixes ---

/// tx:<txid_32> -> serialized TxMeta (height, fee, size, weight, locktime, version)
pub const TX_PREFIX: u8 = b'T';

/// txraw:<txid_32> -> raw transaction bytes
pub const TX_RAW_PREFIX: u8 = b'R';

/// blk:<block_hash_32> -> serialized BlockMeta
pub const BLOCK_PREFIX: u8 = b'B';

/// blkh:<height_u32_be> -> block_hash (32 bytes)
pub const BLOCK_HEIGHT_PREFIX: u8 = b'H';

/// blktxs:<block_hash_32>:<tx_index_u32_be> -> txid (32 bytes)
pub const BLOCK_TXIDS_PREFIX: u8 = b'I';

/// blktxcount:<block_hash_32> -> tx_count (u32 LE)
pub const BLOCK_TX_COUNT_PREFIX: u8 = b'C';

/// addr:<script_hash_32> -> AddressStats (chain_stats)
pub const ADDRESS_PREFIX: u8 = b'A';

/// addrtx:<script_hash_32>:<height_u32_be>:<tx_index_u16_be> -> txid (32 bytes)
pub const ADDRESS_TX_PREFIX: u8 = b'a';

/// utxo:<script_hash_32>:<txid_32>:<vout_u32_be> -> UtxoEntry (value, height)
pub const UTXO_PREFIX: u8 = b'U';

/// spend:<txid_32>:<vout_u32_be> -> spending txid (32 bytes)
pub const SPEND_PREFIX: u8 = b'S';

/// tip -> current chain tip height (u32 LE)
pub const TIP_KEY: &[u8] = b"__TIP__";

/// tip_hash -> current chain tip hash (32 bytes)
pub const TIP_HASH_KEY: &[u8] = b"__TIP_HASH__";

// --- Key builders ---

pub fn tx_key(txid: &[u8; 32]) -> Vec<u8> {
    let mut k = Vec::with_capacity(33);
    k.push(TX_PREFIX);
    k.extend_from_slice(txid);
    k
}

pub fn tx_raw_key(txid: &[u8; 32]) -> Vec<u8> {
    let mut k = Vec::with_capacity(33);
    k.push(TX_RAW_PREFIX);
    k.extend_from_slice(txid);
    k
}

pub fn block_key(hash: &[u8; 32]) -> Vec<u8> {
    let mut k = Vec::with_capacity(33);
    k.push(BLOCK_PREFIX);
    k.extend_from_slice(hash);
    k
}

pub fn block_height_key(height: u32) -> Vec<u8> {
    let mut k = Vec::with_capacity(5);
    k.push(BLOCK_HEIGHT_PREFIX);
    k.extend_from_slice(&height.to_be_bytes());
    k
}

pub fn block_txid_key(block_hash: &[u8; 32], tx_index: u32) -> Vec<u8> {
    let mut k = Vec::with_capacity(37);
    k.push(BLOCK_TXIDS_PREFIX);
    k.extend_from_slice(block_hash);
    k.extend_from_slice(&tx_index.to_be_bytes());
    k
}

pub fn block_tx_count_key(block_hash: &[u8; 32]) -> Vec<u8> {
    let mut k = Vec::with_capacity(33);
    k.push(BLOCK_TX_COUNT_PREFIX);
    k.extend_from_slice(block_hash);
    k
}

pub fn address_key(script_hash: &[u8; 32]) -> Vec<u8> {
    let mut k = Vec::with_capacity(33);
    k.push(ADDRESS_PREFIX);
    k.extend_from_slice(script_hash);
    k
}

pub fn address_tx_key(script_hash: &[u8; 32], height: u32, tx_index: u16) -> Vec<u8> {
    let mut k = Vec::with_capacity(39);
    k.push(ADDRESS_TX_PREFIX);
    k.extend_from_slice(script_hash);
    k.extend_from_slice(&height.to_be_bytes());
    k.extend_from_slice(&tx_index.to_be_bytes());
    k
}

pub fn utxo_key(script_hash: &[u8; 32], txid: &[u8; 32], vout: u32) -> Vec<u8> {
    let mut k = Vec::with_capacity(69);
    k.push(UTXO_PREFIX);
    k.extend_from_slice(script_hash);
    k.extend_from_slice(txid);
    k.extend_from_slice(&vout.to_be_bytes());
    k
}

pub fn spend_key(txid: &[u8; 32], vout: u32) -> Vec<u8> {
    let mut k = Vec::with_capacity(37);
    k.push(SPEND_PREFIX);
    k.extend_from_slice(txid);
    k.extend_from_slice(&vout.to_be_bytes());
    k
}
