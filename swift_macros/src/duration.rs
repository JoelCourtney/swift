use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Meta};

pub fn duration(input: DeriveInput) -> TokenStream {
    let mut duration = quote! { swift::duration::Duration::zero() };

    for attr in input.attrs {
        match attr.meta {
            Meta::NameValue(nv) if nv.path.is_ident("duration") => match nv.value {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) => {
                    duration = s.parse().unwrap();
                }
                _ => panic!("duration attribute must be a string"),
            },
            _ => {}
        }
    }

    let item_ident = input.ident;

    quote! {
        impl Durative for #item_ident {
            fn duration(&self) -> swift::duration::Duration {
                #duration
            }
        }
    }
}
