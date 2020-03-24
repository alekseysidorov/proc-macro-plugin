use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{DeriveInput, Meta};

#[derive(Debug, Default, FromMeta)]
struct Attrs {
    pretty: bool,
}

pub fn impl_codec(input: TokenStream, meta: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse2(input).unwrap();
    let meta: Option<Meta> = if meta.is_empty() {
        None
    } else {
        Some(syn::parse2(meta).unwrap())
    };

    implement_codec(input, meta).into_token_stream()
}

fn implement_codec(input: DeriveInput, params: Option<Meta>) -> impl ToTokens {
    let ident = &input.ident;

    let attrs = params
        .as_ref()
        .map(|meta| Attrs::from_meta(meta).unwrap())
        .unwrap_or_default();

    let to_string_method = if attrs.pretty {
        quote! { to_string_pretty }
    } else {
        quote! { to_string }
    };

    quote! {
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
    }
}
