extern crate proc_macro;

use darling::FromMeta;
use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{parse_macro_input, Attribute, DeriveInput, Meta};

use std::path::Path;

use crate::wasm::WasmMacro;

mod wasm;

fn find_meta_attrs(name: &str, args: &[Attribute]) -> Option<syn::NestedMeta> {
    args.as_ref()
        .iter()
        .filter_map(|a| a.parse_meta().ok())
        .find(|m| m.path().is_ident(name))
        .map(syn::NestedMeta::from)
}

#[derive(Debug, FromMeta)]
struct TextMessageAttrs {
    codec: String,
    #[darling(default)]
    params: Option<Meta>,
}

impl TextMessageAttrs {
    fn from_raw(attrs: &[Attribute]) -> Result<Self, darling::Error> {
        let meta = find_meta_attrs("text_message", attrs).unwrap();
        Self::from_nested_meta(&meta)
    }
}

#[proc_macro_derive(TextMessage, attributes(text_message))]
pub fn text_message(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);

    let attrs =
        TextMessageAttrs::from_raw(&input.attrs).expect("Unable to parse text message attributes.");

    let params = attrs
        .params
        .map(ToTokens::into_token_stream)
        .unwrap_or_default();

    let codec_dir = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("codecs");
    let plugin_name = format!("{}_text_codec.wasm", attrs.codec);
    let codec_path = codec_dir.join(plugin_name);

    let wasm_macro = WasmMacro::from_file(codec_path).expect("Unable to load wasm module");
    wasm_macro
        .proc_macro_attribute(
            "impl_codec",
            input.into_token_stream().into(),
            params.into(),
        )
        .expect("Unable to apply proc_macro_attribute")
}
