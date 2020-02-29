extern crate proc_macro;

use darling::FromMeta;
use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{parse_macro_input, Attribute, DeriveInput, Meta};
use watt::WasmMacro;

static MACRO: WasmMacro = WasmMacro::new(WASM);
static WASM: &[u8] = include_bytes!("../../codecs/serde-json/target/wasm32-unknown-unknown/release/serde_json_text_codec.wasm");
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

    match attrs.codec.as_ref() {
        "serde_json" => {
            // serde_json_text_codec::impl_codec(input.to_token_stream().into(), params.into()).into()
            MACRO.proc_macro_attribute("impl_codec", input.to_token_stream().into(), params.into())
        }
        other => panic!("Unknown test codec: `{}`", other),
    }
}
