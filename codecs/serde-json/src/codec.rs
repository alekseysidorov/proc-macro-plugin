use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{Attribute, DeriveInput};

fn find_meta_attrs(name: &str, args: &[Attribute]) -> Option<syn::NestedMeta> {
    args.as_ref()
        .iter()
        .filter_map(|a| a.parse_meta().ok())
        .find(|m| m.path().is_ident(name))
        .map(syn::NestedMeta::from)
}

#[derive(Debug, Default, FromMeta)]
struct Attrs {
    pretty: bool,
}

#[derive(Debug, FromMeta)]
struct TextMessageAttrs {
    codec: String,
    #[darling(default)]
    params: Option<Attrs>,
}

impl TextMessageAttrs {
    fn from_raw(attrs: &[Attribute]) -> Result<Self, darling::Error> {
        let meta = find_meta_attrs("text_message", attrs).unwrap();
        Self::from_nested_meta(&meta)
    }
}

pub fn implement_codec(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse2(input).unwrap();
    let ident = &input.ident;

    let attrs = TextMessageAttrs::from_raw(&input.attrs)
        .expect("Unable to parse text message attributes.")
        .params
        .unwrap_or_default();

    let to_string_method = if attrs.pretty {
        quote! { to_string_pretty }
    } else {
        quote! { to_string }
    };

    let out = quote! {
        impl std::fmt::Display for #ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let out = serde_json::#to_string_method(self).map_err(|_| std::fmt::Error)?;
                f.write_str(&out)
            }
        }

        impl std::str::FromStr for #ident {
            type Err = serde_json::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                serde_json::from_str(s)
            }
        }
    };
    out.into_token_stream()
}
