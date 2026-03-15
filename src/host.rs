//! Metashrew host function bindings.
//!
//! In WASM mode: extern "C" functions provided by the qubitcoind runtime.
//! In test mode: backed by a global in-memory HashMap (like metashrew-core).

use std::collections::HashMap;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Global in-memory store (always available, used in test mode and for flush)
// ---------------------------------------------------------------------------

static mut CACHE: Option<HashMap<Vec<u8>, Vec<u8>>> = None;
static mut TO_FLUSH: Option<Vec<(Vec<u8>, Vec<u8>)>> = None;
static mut INPUT_DATA: Option<Vec<u8>> = None;

/// Initialize the global cache. Idempotent.
pub fn initialize() {
    unsafe {
        if CACHE.is_none() {
            CACHE = Some(HashMap::new());
            TO_FLUSH = Some(Vec::new());
            INPUT_DATA = Some(Vec::new());
        }
    }
}

/// Clear all state (cache, flush queue, input). Used between tests.
#[allow(static_mut_refs)]
pub fn clear() {
    unsafe {
        CACHE = Some(HashMap::new());
        TO_FLUSH = Some(Vec::new());
        INPUT_DATA = Some(Vec::new());
    }
}

/// Set the input data (for testing: simulates what the host provides).
#[allow(static_mut_refs)]
pub fn set_input(data: Vec<u8>) {
    initialize();
    unsafe {
        *INPUT_DATA.as_mut().unwrap() = data;
    }
}

/// Read a value from the cache by key.
#[allow(static_mut_refs)]
pub fn cache_get(key: &[u8]) -> Option<Vec<u8>> {
    initialize();
    unsafe { CACHE.as_ref().unwrap().get(key).cloned() }
}

/// Set a value in the cache.
#[allow(static_mut_refs)]
pub fn cache_set(key: Vec<u8>, value: Vec<u8>) {
    initialize();
    unsafe {
        CACHE.as_mut().unwrap().insert(key, value);
    }
}

/// Get a reference to the full cache (for test assertions).
#[allow(static_mut_refs)]
pub fn get_cache() -> &'static HashMap<Vec<u8>, Vec<u8>> {
    initialize();
    unsafe { CACHE.as_ref().unwrap() }
}

// ---------------------------------------------------------------------------
// WASM host function extern declarations (only used in non-test WASM builds)
// ---------------------------------------------------------------------------

#[cfg(not(test))]
extern "C" {
    fn __host_len() -> i32;
    fn __load_input(ptr: *mut u8);
    fn __get(key_ptr: *const u8, value_ptr: *mut u8);
    fn __get_len(key_ptr: *const u8) -> i32;
    fn __flush(data_ptr: *const u8);
    fn __log(ptr: *const u8);
}

// ---------------------------------------------------------------------------
// Public API — delegates to extern (WASM) or cache (test)
// ---------------------------------------------------------------------------

/// Allocate an ArrayBuffer in memory.
///
/// Layout: `[4-byte LE length][data]`
/// Returns pointer to the data (after the length prefix).
pub fn alloc_arraybuffer(data: &[u8]) -> *const u8 {
    let len = data.len() as u32;
    let layout = std::alloc::Layout::from_size_align(4 + data.len(), 4).unwrap();
    unsafe {
        let ptr = std::alloc::alloc(layout);
        std::ptr::copy_nonoverlapping(len.to_le_bytes().as_ptr(), ptr, 4);
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr.add(4), data.len());
        ptr.add(4)
    }
}

/// Read the input data.
#[allow(static_mut_refs)]
pub fn load_input() -> Vec<u8> {
    #[cfg(test)]
    {
        initialize();
        unsafe { INPUT_DATA.as_ref().unwrap().clone() }
    }
    #[cfg(not(test))]
    {
        unsafe {
            let len = __host_len() as usize;
            let mut buf = vec![0u8; len];
            __load_input(buf.as_mut_ptr());
            buf
        }
    }
}

/// Read a value from storage by key.
#[allow(static_mut_refs)]
pub fn get(key: &[u8]) -> Option<Vec<u8>> {
    #[cfg(test)]
    {
        cache_get(key)
    }
    #[cfg(not(test))]
    {
        let key_ab = alloc_arraybuffer(key);
        let len = unsafe { __get_len(key_ab) };
        if len <= 0 {
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        unsafe {
            __get(key_ab, buf.as_mut_ptr());
        }
        Some(buf)
    }
}

/// Get the length of a value in storage.
pub fn get_len(key: &[u8]) -> i32 {
    #[cfg(test)]
    {
        cache_get(key).map(|v| v.len() as i32).unwrap_or(0)
    }
    #[cfg(not(test))]
    {
        let key_ab = alloc_arraybuffer(key);
        unsafe { __get_len(key_ab) }
    }
}

/// Flush key-value pairs to storage.
///
/// In test mode: decodes the protobuf and writes directly to the cache.
/// In WASM mode: passes through to the host __flush.
#[allow(static_mut_refs)]
pub fn flush(data: &[u8]) {
    #[cfg(test)]
    {
        use prost::Message;
        if let Ok(msg) = crate::proto::KeyValueFlush::decode(data) {
            let list = &msg.list;
            let mut i = 0;
            while i + 1 < list.len() {
                cache_set(list[i].clone(), list[i + 1].clone());
                i += 2;
            }
        }
    }
    #[cfg(not(test))]
    {
        let data_ab = alloc_arraybuffer(data);
        unsafe { __flush(data_ab) }
    }
}

/// Log a message.
pub fn log(msg: &str) {
    #[cfg(test)]
    {
        eprintln!("[esplorashrew] {}", msg);
    }
    #[cfg(not(test))]
    {
        let msg_ab = alloc_arraybuffer(msg.as_bytes());
        unsafe { __log(msg_ab) }
    }
}
