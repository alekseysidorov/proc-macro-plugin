pub use wasmtime_glue::{toy_alloc, toy_free};

use proc_macro2::TokenStream;
use wasmtime_glue::{str_from_raw_parts, to_host_buf};

use std::str::FromStr;

mod codec;

#[no_mangle]
pub unsafe extern "C" fn implement_codec(
    item_ptr: i32,
    item_len: i32,
) -> i32 {
    let item = str_from_raw_parts(item_ptr, item_len);
    let item = TokenStream::from_str(&item).expect("Unable to parse item");

    let tokens = codec::implement_codec(item);
    let out = tokens.to_string();
    
    to_host_buf(out)
}
