use crate::model::{Model, ResourceSelection};
use proc_macro2::Ident;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{braced, Token, Visibility};

impl Parse for Model {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let visibility: Visibility = input.parse()?;
        let name: Ident = input.parse()?;

        let body;
        braced!(body in input);

        let resources =
            Punctuated::<ResourceSelection, Token![,]>::parse_terminated(&body)?.into_iter();

        Ok(Model {
            visibility,
            name,
            resources: resources.collect(),
        })
    }
}

impl Parse for ResourceSelection {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(ResourceSelection {
            field: input.parse()?,
            _colon: input.parse()?,
            path: input.parse()?,
        })
    }
}
