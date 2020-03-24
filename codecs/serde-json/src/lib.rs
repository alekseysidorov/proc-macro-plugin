pub use wasmtime_glue::{toy_alloc, toy_free};

use proc_macro2::TokenStream;
use wasmtime_glue::{str_from_raw_parts, to_host_buf};

use std::str::FromStr;

mod codec;

#[no_mangle]
pub unsafe extern "C" fn impl_codec(
    attr_ptr: i32,
    attr_len: i32,
    item_ptr: i32,
    item_len: i32,
) -> i32 {
    let attr = str_from_raw_parts(attr_ptr, attr_len);
    let item = str_from_raw_parts(item_ptr, item_len);

    let attr = TokenStream::from_str(&attr).expect("Unable to parse attributes");
    let item = TokenStream::from_str(&item).expect("Unable to parse item");

    let tokens = codec::impl_codec(attr, item);

    let out = tokens.to_string();
    to_host_buf(out)
}
