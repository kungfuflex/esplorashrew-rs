//! View functions exposed as WASM exports.
//!
//! Each view function is called by the qubitcoind runtime via `call_view()`.
//! Input is provided via `__host_len`/`__load_input`, and the result is
//! returned as a pointer to an ArrayBuffer.
//!
//! Function naming follows metashrew convention: all lowercase, single word.

use crate::block::{self, to_hex, to_hex_rev};
use crate::host;
use crate::keys;
use crate::types::*;

/// Helper: read input as a string.
fn input_string() -> String {
    let data = host::load_input();
    String::from_utf8_lossy(&data).to_string()
}

/// Helper: return a JSON response as an ArrayBuffer pointer.
fn json_response(value: &serde_json::Value) -> *const u8 {
    let json = serde_json::to_vec(value).unwrap_or_default();
    host::alloc_arraybuffer(&json)
}

/// Helper: return a raw string response.
fn string_response(s: &str) -> *const u8 {
    host::alloc_arraybuffer(s.as_bytes())
}

/// Helper: return raw bytes.
fn bytes_response(data: &[u8]) -> *const u8 {
    host::alloc_arraybuffer(data)
}

/// Helper: decode a hex txid/hash string to 32 bytes (reversed for internal use).
fn decode_hash(hex: &str) -> Option<[u8; 32]> {
    let hex = hex.trim();
    if hex.len() != 64 {
        return None;
    }
    let bytes: Result<Vec<u8>, _> = (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect();
    let bytes = bytes.ok()?;
    let mut result = [0u8; 32];
    // Reverse for internal storage (Bitcoin hashes are displayed reversed).
    for (i, b) in bytes.iter().rev().enumerate() {
        result[i] = *b;
    }
    Some(result)
}

// =========================================================================
// View functions — metashrew style: all lowercase, single word
// =========================================================================

/// GET /tx/:txid
/// Input: txid hex string
/// Returns: JSON transaction object
#[no_mangle]
pub extern "C" fn tx() -> *const u8 {
    let txid_hex = input_string();
    let txid = match decode_hash(&txid_hex) {
        Some(h) => h,
        None => return json_response(&serde_json::json!({"error": "invalid txid"})),
    };

    // Look up tx metadata.
    let meta_bytes = match host::get(&keys::tx_key(&txid)) {
        Some(b) => b,
        None => return json_response(&serde_json::json!({"error": "tx not found"})),
    };
    let meta: TxMeta = match serde_json::from_slice(&meta_bytes) {
        Ok(m) => m,
        Err(_) => return json_response(&serde_json::json!({"error": "corrupt tx metadata"})),
    };

    // Look up raw tx for full details.
    let raw = host::get(&keys::tx_raw_key(&txid)).unwrap_or_default();

    // Parse the raw tx for vin/vout details.
    let (parsed_tx, _) = block::parse_transaction(&raw);

    let vin: Vec<serde_json::Value> = parsed_tx
        .inputs
        .iter()
        .map(|inp| {
            serde_json::json!({
                "txid": to_hex_rev(&inp.prev_txid),
                "vout": inp.prev_vout,
                "scriptsig": to_hex(&inp.script_sig),
                "sequence": inp.sequence,
                "witness": inp.witness.iter().map(|w| to_hex(w)).collect::<Vec<_>>(),
            })
        })
        .collect();

    let vout: Vec<serde_json::Value> = parsed_tx
        .outputs
        .iter()
        .enumerate()
        .map(|(_, out)| {
            serde_json::json!({
                "scriptpubkey": to_hex(&out.script_pubkey),
                "value": out.value,
            })
        })
        .collect();

    let result = serde_json::json!({
        "txid": txid_hex.trim(),
        "version": meta.version,
        "locktime": meta.locktime,
        "size": meta.size,
        "weight": meta.weight,
        "fee": meta.fee,
        "vin": vin,
        "vout": vout,
        "status": {
            "confirmed": true,
            "block_height": meta.block_height,
            "block_hash": meta.block_hash,
            "block_time": meta.block_time,
        },
    });

    json_response(&result)
}

/// GET /tx/:txid/hex
/// Input: txid hex string
/// Returns: hex-encoded raw transaction
#[no_mangle]
pub extern "C" fn txhex() -> *const u8 {
    let txid_hex = input_string();
    let txid = match decode_hash(&txid_hex) {
        Some(h) => h,
        None => return string_response("invalid txid"),
    };
    match host::get(&keys::tx_raw_key(&txid)) {
        Some(raw) => {
            let hex: String = raw.iter().map(|b| format!("{:02x}", b)).collect();
            string_response(&hex)
        }
        None => string_response("tx not found"),
    }
}

/// GET /tx/:txid/raw
/// Input: txid hex string
/// Returns: raw transaction bytes
#[no_mangle]
pub extern "C" fn txraw() -> *const u8 {
    let txid_hex = input_string();
    let txid = match decode_hash(&txid_hex) {
        Some(h) => h,
        None => return bytes_response(b"invalid txid"),
    };
    match host::get(&keys::tx_raw_key(&txid)) {
        Some(raw) => bytes_response(&raw),
        None => bytes_response(b"tx not found"),
    }
}

/// GET /tx/:txid/status
/// Input: txid hex string
/// Returns: JSON confirmation status
#[no_mangle]
pub extern "C" fn txstatus() -> *const u8 {
    let txid_hex = input_string();
    let txid = match decode_hash(&txid_hex) {
        Some(h) => h,
        None => return json_response(&serde_json::json!({"error": "invalid txid"})),
    };
    match host::get(&keys::tx_key(&txid)) {
        Some(meta_bytes) => {
            if let Ok(meta) = serde_json::from_slice::<TxMeta>(&meta_bytes) {
                json_response(&serde_json::json!({
                    "confirmed": true,
                    "block_height": meta.block_height,
                    "block_hash": meta.block_hash,
                    "block_time": meta.block_time,
                }))
            } else {
                json_response(&serde_json::json!({"confirmed": false}))
            }
        }
        None => json_response(&serde_json::json!({"confirmed": false})),
    }
}

/// GET /tx/:txid/outspend/:vout
/// Input: JSON {"txid": "...", "vout": N}
/// Returns: JSON spending status
#[no_mangle]
pub extern "C" fn txoutspend() -> *const u8 {
    let input = input_string();
    let params: serde_json::Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return json_response(&serde_json::json!({"error": "invalid input"})),
    };

    let txid_hex = params["txid"].as_str().unwrap_or("");
    let vout = params["vout"].as_u64().unwrap_or(0) as u32;
    let txid = match decode_hash(txid_hex) {
        Some(h) => h,
        None => return json_response(&serde_json::json!({"error": "invalid txid"})),
    };

    match host::get(&keys::spend_key(&txid, vout)) {
        Some(spending_txid) => {
            let mut stxid = [0u8; 32];
            if spending_txid.len() == 32 {
                stxid.copy_from_slice(&spending_txid);
            }
            let spending_hex = to_hex_rev(&stxid);

            // Look up the spending tx for confirmation status.
            let status = host::get(&keys::tx_key(&stxid))
                .and_then(|b| serde_json::from_slice::<TxMeta>(&b).ok())
                .map(|meta| {
                    serde_json::json!({
                        "confirmed": true,
                        "block_height": meta.block_height,
                        "block_hash": meta.block_hash,
                        "block_time": meta.block_time,
                    })
                });

            json_response(&serde_json::json!({
                "spent": true,
                "txid": spending_hex,
                "status": status,
            }))
        }
        None => json_response(&serde_json::json!({"spent": false})),
    }
}

/// GET /block/:hash
/// Input: block hash hex string
/// Returns: JSON block metadata
#[no_mangle]
pub extern "C" fn block() -> *const u8 {
    let hash_hex = input_string();
    let hash = match decode_hash(&hash_hex) {
        Some(h) => h,
        None => return json_response(&serde_json::json!({"error": "invalid block hash"})),
    };

    match host::get(&keys::block_key(&hash)) {
        Some(meta_bytes) => {
            if let Ok(meta) = serde_json::from_slice::<BlockMeta>(&meta_bytes) {
                json_response(&serde_json::json!(meta))
            } else {
                json_response(&serde_json::json!({"error": "corrupt block metadata"}))
            }
        }
        None => json_response(&serde_json::json!({"error": "block not found"})),
    }
}

/// GET /block/:hash/status
/// Input: block hash hex string
/// Returns: JSON block status
#[no_mangle]
pub extern "C" fn blockstatus() -> *const u8 {
    let hash_hex = input_string();
    let hash = match decode_hash(&hash_hex) {
        Some(h) => h,
        None => return json_response(&serde_json::json!({"error": "invalid block hash"})),
    };

    match host::get(&keys::block_key(&hash)) {
        Some(meta_bytes) => {
            if let Ok(meta) = serde_json::from_slice::<BlockMeta>(&meta_bytes) {
                // Check if next block exists.
                let next_hash = host::get(&keys::block_height_key(meta.height + 1))
                    .map(|h| {
                        let mut arr = [0u8; 32];
                        if h.len() == 32 {
                            arr.copy_from_slice(&h);
                        }
                        to_hex_rev(&arr)
                    });

                json_response(&serde_json::json!({
                    "in_best_chain": true,
                    "height": meta.height,
                    "next_best": next_hash,
                }))
            } else {
                json_response(&serde_json::json!({"error": "corrupt block"}))
            }
        }
        None => json_response(&serde_json::json!({"error": "block not found"})),
    }
}

/// GET /block/:hash/txids
/// Input: block hash hex string
/// Returns: JSON array of txid strings
#[no_mangle]
pub extern "C" fn blocktxids() -> *const u8 {
    let hash_hex = input_string();
    let hash = match decode_hash(&hash_hex) {
        Some(h) => h,
        None => return json_response(&serde_json::json!({"error": "invalid block hash"})),
    };

    // Get tx count.
    let tx_count = host::get(&keys::block_tx_count_key(&hash))
        .and_then(|b| {
            if b.len() >= 4 {
                Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            } else {
                None
            }
        })
        .unwrap_or(0);

    let mut txids = Vec::with_capacity(tx_count as usize);
    for i in 0..tx_count {
        if let Some(txid_bytes) = host::get(&keys::block_txid_key(&hash, i)) {
            let mut txid = [0u8; 32];
            if txid_bytes.len() == 32 {
                txid.copy_from_slice(&txid_bytes);
            }
            txids.push(to_hex_rev(&txid));
        }
    }

    json_response(&serde_json::json!(txids))
}

/// GET /block/:hash/header
/// Input: block hash hex string
/// Returns: hex-encoded 80-byte block header
#[no_mangle]
pub extern "C" fn blockheader() -> *const u8 {
    // For the header we'd need to store the raw header.
    // For now, return the block hash as a placeholder.
    let hash_hex = input_string();
    string_response(&hash_hex)
}

/// GET /block-height/:height
/// Input: height as decimal string
/// Returns: block hash hex string
#[no_mangle]
pub extern "C" fn blockheight() -> *const u8 {
    let input = input_string();
    let height: u32 = match input.trim().parse() {
        Ok(h) => h,
        Err(_) => return string_response("invalid height"),
    };

    match host::get(&keys::block_height_key(height)) {
        Some(hash_bytes) => {
            let mut hash = [0u8; 32];
            if hash_bytes.len() == 32 {
                hash.copy_from_slice(&hash_bytes);
            }
            string_response(&to_hex_rev(&hash))
        }
        None => string_response("block not found"),
    }
}

/// GET /blocks/tip/height
/// Returns: current chain tip height as decimal string
#[no_mangle]
pub extern "C" fn tipheight() -> *const u8 {
    let height = host::get(keys::TIP_KEY)
        .and_then(|b| {
            if b.len() >= 4 {
                Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            } else {
                None
            }
        })
        .unwrap_or(0);
    string_response(&height.to_string())
}

/// GET /blocks/tip/hash
/// Returns: current chain tip block hash hex string
#[no_mangle]
pub extern "C" fn tiphash() -> *const u8 {
    match host::get(keys::TIP_HASH_KEY) {
        Some(hash_bytes) => {
            let mut hash = [0u8; 32];
            if hash_bytes.len() == 32 {
                hash.copy_from_slice(&hash_bytes);
            }
            string_response(&to_hex_rev(&hash))
        }
        None => string_response(""),
    }
}

/// GET /scripthash/:hash/utxo
/// Input: script_hash hex string (SHA-256 of scriptPubKey)
/// Returns: JSON array of UTXO objects
#[no_mangle]
pub extern "C" fn utxosbyscripthash() -> *const u8 {
    let input = input_string();
    let sh = match decode_hash(input.trim()) {
        Some(h) => h,
        None => return json_response(&serde_json::json!({"error": "invalid script hash"})),
    };

    // Scan UTXOs for this script hash.
    // The UTXO key prefix is: U + script_hash(32)
    let _prefix_key = keys::address_key(&sh);
    // We'd need prefix iteration from the host, which isn't available.
    // For now, return what we can from direct lookups.
    // TODO: Implement prefix scan in host ABI or store UTXO list per address.

    json_response(&serde_json::json!([]))
}
