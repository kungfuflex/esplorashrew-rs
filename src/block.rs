//! Minimal block/transaction parser.
//!
//! Parses raw Bitcoin block data without depending on the full bitcoin crate
//! (which may not compile to wasm32-unknown-unknown cleanly). Uses manual
//! deserialization of the wire format.

use sha2::{Digest, Sha256};

/// Parsed block header.
#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub version: i32,
    pub prev_block: [u8; 32],
    pub merkle_root: [u8; 32],
    pub time: u32,
    pub bits: u32,
    pub nonce: u32,
    pub hash: [u8; 32],
}

/// Parsed transaction.
#[derive(Debug, Clone)]
pub struct Transaction {
    pub version: i32,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
    pub locktime: u32,
    pub txid: [u8; 32],
    pub wtxid: [u8; 32],
    /// Raw bytes of this transaction.
    pub raw: Vec<u8>,
    /// Size in bytes.
    pub size: u32,
    /// Weight units.
    pub weight: u32,
}

/// Transaction input.
#[derive(Debug, Clone)]
pub struct TxInput {
    pub prev_txid: [u8; 32],
    pub prev_vout: u32,
    pub script_sig: Vec<u8>,
    pub sequence: u32,
    pub witness: Vec<Vec<u8>>,
}

/// Transaction output.
#[derive(Debug, Clone)]
pub struct TxOutput {
    pub value: u64,
    pub script_pubkey: Vec<u8>,
}

/// Parsed block.
#[derive(Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    /// Raw header bytes (80 bytes).
    pub raw_header: Vec<u8>,
}

/// Double SHA-256 hash.
pub fn hash256(data: &[u8]) -> [u8; 32] {
    let first = Sha256::digest(data);
    let second = Sha256::digest(&first);
    let mut result = [0u8; 32];
    result.copy_from_slice(&second);
    result
}

/// Read a compact size (varint) from a byte slice.
/// Returns (value, bytes_consumed).
pub fn read_compact_size(data: &[u8]) -> (u64, usize) {
    if data.is_empty() {
        return (0, 0);
    }
    match data[0] {
        0..=252 => (data[0] as u64, 1),
        253 => {
            let v = u16::from_le_bytes([data[1], data[2]]) as u64;
            (v, 3)
        }
        254 => {
            let v = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as u64;
            (v, 5)
        }
        255 => {
            let v = u64::from_le_bytes([
                data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8],
            ]);
            (v, 9)
        }
    }
}

/// Parse a block header from raw bytes.
pub fn parse_header(data: &[u8]) -> BlockHeader {
    let version = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let mut prev_block = [0u8; 32];
    prev_block.copy_from_slice(&data[4..36]);
    let mut merkle_root = [0u8; 32];
    merkle_root.copy_from_slice(&data[36..68]);
    let time = u32::from_le_bytes([data[68], data[69], data[70], data[71]]);
    let bits = u32::from_le_bytes([data[72], data[73], data[74], data[75]]);
    let nonce = u32::from_le_bytes([data[76], data[77], data[78], data[79]]);
    let hash = hash256(&data[..80]);

    BlockHeader {
        version,
        prev_block,
        merkle_root,
        time,
        bits,
        nonce,
        hash,
    }
}

/// Parse a full block from raw bytes.
pub fn parse_block(data: &[u8]) -> Block {
    let header = parse_header(data);
    let raw_header = data[..80].to_vec();

    let mut offset = 80;
    let (tx_count, consumed) = read_compact_size(&data[offset..]);
    offset += consumed;

    let mut transactions = Vec::with_capacity(tx_count as usize);
    for _ in 0..tx_count {
        let (tx, consumed) = parse_transaction(&data[offset..]);
        transactions.push(tx);
        offset += consumed;
    }

    Block {
        header,
        transactions,
        raw_header,
    }
}

/// Parse a transaction from raw bytes.
/// Returns (Transaction, bytes_consumed).
pub fn parse_transaction(data: &[u8]) -> (Transaction, usize) {
    let start = 0;
    let mut offset = 0;

    let version = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    offset += 4;

    // Check for segwit marker.
    let has_witness = data[offset] == 0x00 && data[offset + 1] != 0x00;
    let mut witness_offset = 0;
    if has_witness {
        offset += 2; // skip marker and flag
    }

    // Parse inputs.
    let (input_count, consumed) = read_compact_size(&data[offset..]);
    offset += consumed;

    let mut inputs = Vec::with_capacity(input_count as usize);
    for _ in 0..input_count {
        let mut prev_txid = [0u8; 32];
        prev_txid.copy_from_slice(&data[offset..offset + 32]);
        offset += 32;
        let prev_vout = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;
        let (script_len, consumed) = read_compact_size(&data[offset..]);
        offset += consumed;
        let script_sig = data[offset..offset + script_len as usize].to_vec();
        offset += script_len as usize;
        let sequence = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        inputs.push(TxInput {
            prev_txid,
            prev_vout,
            script_sig,
            sequence,
            witness: Vec::new(),
        });
    }

    // Parse outputs.
    let (output_count, consumed) = read_compact_size(&data[offset..]);
    offset += consumed;

    let mut outputs = Vec::with_capacity(output_count as usize);
    for _ in 0..output_count {
        let value = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;
        let (script_len, consumed) = read_compact_size(&data[offset..]);
        offset += consumed;
        let script_pubkey = data[offset..offset + script_len as usize].to_vec();
        offset += script_len as usize;

        outputs.push(TxOutput {
            value,
            script_pubkey,
        });
    }

    // Parse witness data if present.
    if has_witness {
        for input in &mut inputs {
            let (witness_count, consumed) = read_compact_size(&data[offset..]);
            offset += consumed;
            for _ in 0..witness_count {
                let (item_len, consumed) = read_compact_size(&data[offset..]);
                offset += consumed;
                let item = data[offset..offset + item_len as usize].to_vec();
                offset += item_len as usize;
                input.witness.push(item);
            }
        }
    }

    // Locktime.
    let locktime = u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]);
    offset += 4;

    let raw = data[start..offset].to_vec();
    let size = raw.len() as u32;

    // Compute txid (hash of non-witness serialization).
    let txid = if has_witness {
        // Rebuild non-witness serialization.
        let mut nosw = Vec::new();
        nosw.extend_from_slice(&data[0..4]); // version
        // Skip marker+flag, build from parsed inputs/outputs.
        let input_data_start = 6; // version(4) + marker(1) + flag(1)
        // Actually, easier to re-serialize without witness.
        nosw.extend_from_slice(&encode_compact_size(input_count));
        let mut pos = input_data_start;
        // Re-parse to find byte ranges... simpler to hash from components.
        // Let's just build it from parsed data.
        for inp in &inputs {
            nosw.extend_from_slice(&inp.prev_txid);
            nosw.extend_from_slice(&inp.prev_vout.to_le_bytes());
            nosw.extend_from_slice(&encode_compact_size(inp.script_sig.len() as u64));
            nosw.extend_from_slice(&inp.script_sig);
            nosw.extend_from_slice(&inp.sequence.to_le_bytes());
        }
        nosw.extend_from_slice(&encode_compact_size(output_count));
        for out in &outputs {
            nosw.extend_from_slice(&out.value.to_le_bytes());
            nosw.extend_from_slice(&encode_compact_size(out.script_pubkey.len() as u64));
            nosw.extend_from_slice(&out.script_pubkey);
        }
        nosw.extend_from_slice(&locktime.to_le_bytes());
        hash256(&nosw)
    } else {
        hash256(&raw)
    };

    // wtxid is hash of full serialization (including witness).
    let wtxid = hash256(&raw);

    // Weight = base_size * 3 + total_size
    let base_size = if has_witness {
        // Approximate base size = total - witness_data - 2 (marker+flag)
        // More accurate: sum of non-witness fields
        let mut bs = 4u32; // version
        bs += compact_size_len(input_count) as u32;
        for inp in &inputs {
            bs += 32 + 4; // prev_txid + prev_vout
            bs += compact_size_len(inp.script_sig.len() as u64) as u32;
            bs += inp.script_sig.len() as u32;
            bs += 4; // sequence
        }
        bs += compact_size_len(output_count) as u32;
        for out in &outputs {
            bs += 8; // value
            bs += compact_size_len(out.script_pubkey.len() as u64) as u32;
            bs += out.script_pubkey.len() as u32;
        }
        bs += 4; // locktime
        bs
    } else {
        size
    };
    let weight = base_size * 3 + size;

    let tx = Transaction {
        version,
        inputs,
        outputs,
        locktime,
        txid,
        wtxid,
        raw,
        size,
        weight,
    };

    (tx, offset)
}

/// Encode a compact size integer.
pub fn encode_compact_size(n: u64) -> Vec<u8> {
    if n < 253 {
        vec![n as u8]
    } else if n <= 0xffff {
        let mut v = vec![253u8];
        v.extend_from_slice(&(n as u16).to_le_bytes());
        v
    } else if n <= 0xffffffff {
        let mut v = vec![254u8];
        v.extend_from_slice(&(n as u32).to_le_bytes());
        v
    } else {
        let mut v = vec![255u8];
        v.extend_from_slice(&n.to_le_bytes());
        v
    }
}

/// Returns the byte length of a compact size encoding.
fn compact_size_len(n: u64) -> usize {
    if n < 253 {
        1
    } else if n <= 0xffff {
        3
    } else if n <= 0xffffffff {
        5
    } else {
        9
    }
}

/// Hex-encode bytes (lowercase).
pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Hex-encode bytes in reverse order (for display hashes).
pub fn to_hex_rev(bytes: &[u8]) -> String {
    bytes.iter().rev().map(|b| format!("{:02x}", b)).collect()
}

/// Compute the script hash (SHA-256 of the scriptPubKey).
/// This is what Esplora uses for the /scripthash/ endpoints.
pub fn script_hash(script_pubkey: &[u8]) -> [u8; 32] {
    let hash = Sha256::digest(script_pubkey);
    let mut result = [0u8; 32];
    result.copy_from_slice(&hash);
    result
}
