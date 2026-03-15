//! Metashrew host function bindings.
//!
//! These extern functions are provided by the qubitcoind WASM runtime.

extern "C" {
    /// Returns the length of the input data buffer.
    pub fn __host_len() -> i32;
    /// Copies the input data into WASM memory at `ptr`.
    pub fn __load_input(ptr: *mut u8);
    /// Reads the value for `key_ptr` (ArrayBuffer) into `value_ptr`.
    pub fn __get(key_ptr: *const u8, value_ptr: *mut u8);
    /// Returns the length of the value for `key_ptr` (ArrayBuffer).
    pub fn __get_len(key_ptr: *const u8) -> i32;
    /// Flushes a KeyValueFlush protobuf at `data_ptr` (ArrayBuffer) to storage.
    pub fn __flush(data_ptr: *const u8);
    /// Logs a message (ArrayBuffer at `ptr`).
    pub fn __log(ptr: *const u8);
}

/// Read the input data provided by the host.
pub fn load_input() -> Vec<u8> {
    unsafe {
        let len = __host_len() as usize;
        let mut buf = vec![0u8; len];
        __load_input(buf.as_mut_ptr());
        buf
    }
}

/// Allocate an ArrayBuffer in WASM memory.
///
/// Layout: [4-byte LE length][data]
/// Returns pointer to the data (after the length prefix).
pub fn alloc_arraybuffer(data: &[u8]) -> *const u8 {
    let len = data.len() as u32;
    let layout = std::alloc::Layout::from_size_align(4 + data.len(), 4).unwrap();
    unsafe {
        let ptr = std::alloc::alloc(layout);
        // Write length prefix.
        std::ptr::copy_nonoverlapping(len.to_le_bytes().as_ptr(), ptr, 4);
        // Write data.
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr.add(4), data.len());
        // Return pointer to data (host reads length at ptr-4).
        ptr.add(4)
    }
}

/// Read a value from the host storage by key.
pub fn get(key: &[u8]) -> Option<Vec<u8>> {
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

/// Get the length of a value in host storage.
pub fn get_len(key: &[u8]) -> i32 {
    let key_ab = alloc_arraybuffer(key);
    unsafe { __get_len(key_ab) }
}

/// Flush key-value pairs to host storage.
pub fn flush(data: &[u8]) {
    let data_ab = alloc_arraybuffer(data);
    unsafe { __flush(data_ab) }
}

/// Log a message to the host.
pub fn log(msg: &str) {
    let msg_ab = alloc_arraybuffer(msg.as_bytes());
    unsafe { __log(msg_ab) }
}
