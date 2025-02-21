use crate::model::Model;
use proc_macro2::Ident;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{parenthesized, Path, Token, Visibility};

impl Parse for Model {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let visibility: Visibility = input.parse()?;
        let name: Ident = input.parse()?;

        let body;
        parenthesized!(body in input);

        let resources = Punctuated::<Path, Token![,]>::parse_terminated(&body)?.into_iter();

        Ok(Model {
            visibility,
            name,
            resources: resources.collect(),
        })
    }
}
