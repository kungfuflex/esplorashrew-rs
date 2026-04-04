//! View functions exposed as WASM exports.
//!
//! In test mode, the `extern "C"` and `#[no_mangle]` are omitted so the
//! functions can be called as regular Rust fns from tests.
//! Function naming follows metashrew convention: all lowercase, single word.

use crate::block::{self, to_hex, to_hex_rev};
use crate::host;
use crate::keys;
use crate::types::*;

/// In non-test (WASM) builds, apply #[no_mangle] extern "C".
/// In test builds, emit a plain pub fn.
macro_rules! view_fn {
    (
        $(#[doc = $doc:literal])*
        fn $name:ident () -> *const u8
        $body:block
    ) => {
        $(#[doc = $doc])*
        #[cfg(not(test))]
        #[no_mangle]
        pub extern "C" fn $name() -> *const u8 $body

        $(#[doc = $doc])*
        #[cfg(test)]
        pub fn $name() -> *const u8 $body
    };
}

fn input_string() -> String {
    let data = host::load_input();
    // Skip 4-byte height prefix (metashrew ABI: [height_le32 ++ payload])
    let payload = if data.len() > 4 { &data[4..] } else { &data };
    String::from_utf8_lossy(payload).to_string()
}

fn json_response(value: &serde_json::Value) -> *const u8 {
    let json = serde_json::to_vec(value).unwrap_or_default();
    host::alloc_arraybuffer(&json)
}

fn string_response(s: &str) -> *const u8 {
    host::alloc_arraybuffer(s.as_bytes())
}

fn bytes_response(data: &[u8]) -> *const u8 {
    host::alloc_arraybuffer(data)
}

fn decode_hash(hex: &str) -> Option<[u8; 32]> {
    let hex = hex.trim();
    if hex.len() != 64 { return None; }
    let bytes: Result<Vec<u8>, _> = (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect();
    let bytes = bytes.ok()?;
    let mut result = [0u8; 32];
    for (i, b) in bytes.iter().rev().enumerate() {
        result[i] = *b;
    }
    Some(result)
}

// =========================================================================
// View functions
// =========================================================================

view_fn! {
    /// GET /tx/:txid
    fn tx() -> *const u8 {
        let txid_hex = input_string();
        let txid = match decode_hash(&txid_hex) {
            Some(h) => h,
            None => return json_response(&serde_json::json!({"error": "invalid txid"})),
        };
        let meta_bytes = match host::get(&keys::tx_key(&txid)) {
            Some(b) => b,
            None => return json_response(&serde_json::json!({"error": "tx not found"})),
        };
        let meta: TxMeta = match serde_json::from_slice(&meta_bytes) {
            Ok(m) => m,
            Err(_) => return json_response(&serde_json::json!({"error": "corrupt tx metadata"})),
        };
        let raw = host::get(&keys::tx_raw_key(&txid)).unwrap_or_default();
        let (parsed_tx, _) = block::parse_transaction(&raw);
        let vin: Vec<serde_json::Value> = parsed_tx.inputs.iter().map(|inp| {
            serde_json::json!({
                "txid": to_hex_rev(&inp.prev_txid),
                "vout": inp.prev_vout,
                "scriptsig": to_hex(&inp.script_sig),
                "sequence": inp.sequence,
                "witness": inp.witness.iter().map(|w| to_hex(w)).collect::<Vec<_>>(),
            })
        }).collect();
        let vout: Vec<serde_json::Value> = parsed_tx.outputs.iter().enumerate().map(|(_, out)| {
            serde_json::json!({
                "scriptpubkey": to_hex(&out.script_pubkey),
                "value": out.value,
            })
        }).collect();
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
}

view_fn! {
    /// GET /tx/:txid/hex
    fn txhex() -> *const u8 {
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
}

view_fn! {
    /// GET /tx/:txid/raw
    fn txraw() -> *const u8 {
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
}

view_fn! {
    /// GET /tx/:txid/status
    fn txstatus() -> *const u8 {
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
}

view_fn! {
    /// GET /tx/:txid/outspend/:vout
    fn txoutspend() -> *const u8 {
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
                if spending_txid.len() == 32 { stxid.copy_from_slice(&spending_txid); }
                let spending_hex = to_hex_rev(&stxid);
                let status = host::get(&keys::tx_key(&stxid))
                    .and_then(|b| serde_json::from_slice::<TxMeta>(&b).ok())
                    .map(|meta| serde_json::json!({
                        "confirmed": true,
                        "block_height": meta.block_height,
                        "block_hash": meta.block_hash,
                        "block_time": meta.block_time,
                    }));
                json_response(&serde_json::json!({
                    "spent": true,
                    "txid": spending_hex,
                    "status": status,
                }))
            }
            None => json_response(&serde_json::json!({"spent": false})),
        }
    }
}

view_fn! {
    /// GET /block/:hash
    fn block() -> *const u8 {
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
}

view_fn! {
    /// GET /block/:hash/status
    fn blockstatus() -> *const u8 {
        let hash_hex = input_string();
        let hash = match decode_hash(&hash_hex) {
            Some(h) => h,
            None => return json_response(&serde_json::json!({"error": "invalid block hash"})),
        };
        match host::get(&keys::block_key(&hash)) {
            Some(meta_bytes) => {
                if let Ok(meta) = serde_json::from_slice::<BlockMeta>(&meta_bytes) {
                    let next_hash = host::get(&keys::block_height_key(meta.height + 1))
                        .map(|h| {
                            let mut arr = [0u8; 32];
                            if h.len() == 32 { arr.copy_from_slice(&h); }
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
}

view_fn! {
    /// GET /block/:hash/txids
    fn blocktxids() -> *const u8 {
        let hash_hex = input_string();
        let hash = match decode_hash(&hash_hex) {
            Some(h) => h,
            None => return json_response(&serde_json::json!({"error": "invalid block hash"})),
        };
        let tx_count = host::get(&keys::block_tx_count_key(&hash))
            .and_then(|b| if b.len() >= 4 {
                Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            } else { None })
            .unwrap_or(0);
        let mut txids = Vec::with_capacity(tx_count as usize);
        for i in 0..tx_count {
            if let Some(txid_bytes) = host::get(&keys::block_txid_key(&hash, i)) {
                let mut txid = [0u8; 32];
                if txid_bytes.len() == 32 { txid.copy_from_slice(&txid_bytes); }
                txids.push(to_hex_rev(&txid));
            }
        }
        json_response(&serde_json::json!(txids))
    }
}

view_fn! {
    /// GET /block/:hash/header
    fn blockheader() -> *const u8 {
        let hash_hex = input_string();
        string_response(&hash_hex)
    }
}

view_fn! {
    /// GET /block-height/:height
    fn blockheight() -> *const u8 {
        let input = input_string();
        let height: u32 = match input.trim().parse() {
            Ok(h) => h,
            Err(_) => return string_response("invalid height"),
        };
        match host::get(&keys::block_height_key(height)) {
            Some(hash_bytes) => {
                let mut hash = [0u8; 32];
                if hash_bytes.len() == 32 { hash.copy_from_slice(&hash_bytes); }
                string_response(&to_hex_rev(&hash))
            }
            None => string_response("block not found"),
        }
    }
}

view_fn! {
    /// GET /blocks/tip/height
    fn tipheight() -> *const u8 {
        let height = host::get(keys::TIP_KEY)
            .and_then(|b| if b.len() >= 4 {
                Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            } else { None })
            .unwrap_or(0);
        string_response(&height.to_string())
    }
}

view_fn! {
    /// GET /blocks/tip/hash
    fn tiphash() -> *const u8 {
        match host::get(keys::TIP_HASH_KEY) {
            Some(hash_bytes) => {
                let mut hash = [0u8; 32];
                if hash_bytes.len() == 32 { hash.copy_from_slice(&hash_bytes); }
                string_response(&to_hex_rev(&hash))
            }
            None => string_response(""),
        }
    }
}

view_fn! {
    /// GET /scripthash/:hash/utxo
    ///
    /// Returns unspent transaction outputs for the given script hash.
    /// Iterates the UTXO index and filters out spent outputs.
    fn utxosbyscripthash() -> *const u8 {
        let input = input_string();
        let sh = match decode_hash(input.trim()) {
            Some(h) => h,
            None => return json_response(&serde_json::json!({"error": "invalid script hash"})),
        };

        // Read UTXO count for this script hash
        let count_key = crate::keys::utxo_count_key(&sh);
        let count = host::get(&count_key)
            .and_then(|b| if b.len() >= 4 {
                Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            } else { None })
            .unwrap_or(0);

        let mut utxos = Vec::new();

        for i in 0..count {
            let idx_key = crate::keys::utxo_idx_key(&sh, i);
            let idx_value = match host::get(&idx_key) {
                Some(v) if v.len() == 36 => v,
                _ => continue,
            };

            // Parse txid (32 bytes) + vout (4 bytes BE)
            let mut txid = [0u8; 32];
            txid.copy_from_slice(&idx_value[0..32]);
            let vout = u32::from_be_bytes([idx_value[32], idx_value[33], idx_value[34], idx_value[35]]);

            // Check if this output is spent
            let spend = crate::keys::spend_key(&txid, vout);
            if host::get_len(&spend) > 0 {
                continue; // Spent — skip
            }

            // Read the UTXO entry
            let utxo_key = crate::keys::utxo_key(&sh, &txid, vout);
            if let Some(utxo_bytes) = host::get(&utxo_key) {
                if let Ok(entry) = serde_json::from_slice::<crate::types::UtxoEntry>(&utxo_bytes) {
                    utxos.push(serde_json::json!({
                        "txid": entry.txid,
                        "vout": entry.vout,
                        "value": entry.value,
                        "status": {
                            "confirmed": true,
                            "block_height": entry.block_height,
                            "block_hash": entry.block_hash,
                            "block_time": entry.block_time,
                        }
                    }));
                }
            }
        }

        json_response(&serde_json::json!(utxos))
    }
}
