use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{DeriveInput, Meta, NestedMeta};

#[derive(Debug, Default)]
struct Attrs {
    pretty: bool,
}

impl Attrs {
    fn from_meta(meta: &Meta) -> Self {
        let list = match meta {
            Meta::List(list) => {
                list
            },
            other => panic!()
        };

        let meta = list.nested.iter().next().unwrap();
        match meta {
            NestedMeta::Meta(Meta::Path(path)) => Self { pretty: path.is_ident("pretty") },
            other => panic!(),
        }
    }
}

#[no_mangle]
pub extern "C" fn impl_codec(input: TokenStream, meta: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse2(input).unwrap();
    let meta: Option<Meta> = if meta.is_empty() {
        None
    } else {
        Some(syn::parse2(meta).unwrap())
    };

    implement_codec(input, meta).into_token_stream()
}

pub fn implement_codec(input: DeriveInput, params: Option<Meta>) -> impl ToTokens {
    let ident = &input.ident;

    let attrs = params.as_ref().map(Attrs::from_meta).unwrap_or_default();

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
