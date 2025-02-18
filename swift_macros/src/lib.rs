use syn::parse_macro_input;

use crate::activity::{process_activity, Activity};
use crate::model::{process_model, Model};
use proc_macro::TokenStream;

mod activity;
mod model;

/// Creates a model and associated structs from a selection of resources.
///
/// Expects a struct-like item, but without the `struct` keyword. For example:
///
/// ```
/// # fn main() {}
/// # use swift_macros::model;
/// model! {
///     MyModel {
///         res_a: ResourceA,
///         res_b: ResourceB
///     }
/// }
/// ```
#[proc_macro]
pub fn model(input: TokenStream) -> TokenStream {
    let model = parse_macro_input!(input as Model);
    process_model(model).into()
}

#[proc_macro]
pub fn activity(input: TokenStream) -> TokenStream {
    let activity = parse_macro_input!(input as Activity);
    process_activity(activity).into()
}
