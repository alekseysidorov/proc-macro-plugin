use std::{mem, os::raw::c_void};

#[no_mangle]
pub unsafe extern "C" fn toy_alloc(size: i32) -> i32 {
    let size_bytes: [u8; 4] = size.to_le_bytes();
    let mut buf: Vec<u8> = Vec::with_capacity(size as usize + size_bytes.len());
    // First byte is the total size of allocated buffer.
    buf.extend(size_bytes.iter());
    to_host_ptr(buf)
}

#[no_mangle]
pub unsafe extern "C" fn toy_free(ptr: i32) {
    let ptr = ptr as usize as *mut u8;
    let mut size_bytes = [0u8; 4];
    ptr.copy_to(size_bytes.as_mut_ptr(), 4);

    let size = u32::from_le_bytes(size_bytes) as usize;
    Vec::from_raw_parts(ptr, size, size);
}

pub unsafe fn to_host_buf(buf: impl AsRef<[u8]>) -> i32 {
    let buf = buf.as_ref();
    let size = buf.len();
    let size_bytes: [u8; 4] = (size as i32).to_le_bytes();

    let mut host_buf: Vec<u8> = Vec::with_capacity(size + size_bytes.len());
    // First byte is the total size of allocated buffer.
    host_buf.extend(size_bytes.iter());
    host_buf.extend_from_slice(buf);
    to_host_ptr(host_buf)
}

pub unsafe fn str_from_raw_parts<'a>(ptr: i32, len: i32) -> &'a str {
    let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
    std::str::from_utf8(slice).unwrap()
}

unsafe fn to_host_ptr(mut buf: Vec<u8>) -> i32 {
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    ptr as *mut c_void as usize as i32
}
