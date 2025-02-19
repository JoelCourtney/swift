use syn::parse_macro_input;

use crate::activity::{process_activity, Activity};
use crate::model::{process_model, Model};
use proc_macro::TokenStream;

mod activity;
mod model;

#[proc_macro]
pub fn model(input: TokenStream) -> TokenStream {
    let model = parse_macro_input!(input as Model);
    process_model(model).into()
}

/// Implements the `Activity` type for a
#[proc_macro]
pub fn impl_activity(input: TokenStream) -> TokenStream {
    let activity = parse_macro_input!(input as Activity);
    process_activity(activity).into()
}
