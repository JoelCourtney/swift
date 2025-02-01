use quote::{format_ident, quote, ToTokens};
use syn::{parse_macro_input, DeriveInput, Item, ItemTrait, Path, Token, TraitItem};

use proc_macro::TokenStream;

mod duration;
mod operation;

#[proc_macro_derive(Durative, attributes(duration))]
pub fn durative(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    duration::duration(input).into()
}

#[proc_macro]
pub fn operation(input: TokenStream) -> TokenStream {
    operation::process_operation(input.to_string()).into()
}

#[proc_macro]
pub fn identity(input: TokenStream) -> TokenStream {
    input.to_string().parse().unwrap()
}

#[proc_macro]
pub fn extras_module(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);

    let stream: proc_macro2::TokenStream = match input {
        Item::Mod(mut m) => {
            m.ident = format_ident!("{}_extras_module", m.ident);
            quote! { #m }
        }
        _ => unreachable!()
    };


    stream.into()
}

#[proc_macro]
pub fn generate_resource_type_tag(input: TokenStream) -> TokenStream {
    let input: String = input.to_string();
    let colon_index = input.find(':').unwrap();

    let id = input[..colon_index].trim();
    let ty = input[colon_index + 1..].trim();

    let new_ident = format_ident!("{}ResourceTypeTag", id);
    let type_ident = format_ident!("{ty}");

    let result = quote! {
        pub enum #new_ident {}

        impl swift::resource::ResourceTypeTag for #new_ident {
            type ResourceType = #type_ident;
        }
    };
    result.into()
}

#[proc_macro]
pub fn get_resource_type_tag(input: TokenStream) -> TokenStream {
    let input: String = input.to_string();

    let new_ident = format_ident!("{input}ResourceTypeTag");

    (quote! { #new_ident }).into()
}
