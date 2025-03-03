mod input;
mod output;

use proc_macro2::{Ident, TokenStream};
use syn::Path;

#[derive(Debug)]
pub struct Op {
    pub context: Context,
    pub reads: Vec<Ident>,
    pub writes: Vec<Ident>,
    pub read_writes: Vec<Ident>,
    body: TokenStream,
    uuid: String,
}

#[derive(Debug)]
pub enum Context {
    Activity(Path),
    _Arguments(Vec<Ident>),
    None,
}
