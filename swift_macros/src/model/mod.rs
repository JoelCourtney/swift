mod input;
mod output;

use proc_macro2::{Ident, TokenStream};
use quote::ToTokens;
use syn::{Path, Token, Visibility};

pub fn process_model(model: Model) -> TokenStream {
    model.into_token_stream()
}

pub struct Model {
    visibility: Visibility,
    name: Ident,
    resources: Vec<ResourceSelection>,
}

struct ResourceSelection {
    field: Ident,
    _colon: Token![:],
    path: Path,
}
