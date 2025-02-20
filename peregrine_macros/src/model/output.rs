use crate::model::Model;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens, TokenStreamExt};

impl ToTokens for Model {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Model {
            visibility,
            name,
            resources,
        } = self;

        let (resource_names, resource_paths) = resources
            .iter()
            .map(|r| (&r.field, &r.path))
            .collect::<(Vec<_>, Vec<_>)>();
        let timeline_names = resources
            .iter()
            .map(|r| format_ident!("{}_operation_timeline", r.field))
            .collect::<Vec<_>>();
        let history_names = resources
            .iter()
            .map(|r| format_ident!("{}_history", r.field))
            .collect::<Vec<_>>();

        let timelines_name = format_ident!("{name}Timelines");
        let initial_conditions_name = format_ident!("{name}InitialConditions");
        let histories_name = format_ident!("{name}Histories");

        let result = quote! {
            #visibility enum #name {}

            impl<'o> peregrine::Model<'o> for #name {
                type Timelines = #timelines_name<'o>;
                type InitialConditions = #initial_conditions_name<'o>;
                type Histories = #histories_name<'o>;
            }

            #visibility struct #initial_conditions_name<'h> {
                #(#resource_names: <#resource_paths as peregrine::Resource<'h>>::Write,)*
            }

            #visibility struct #timelines_name<'o> {
                #(#timeline_names: peregrine::timeline::Timeline<'o, #resource_paths, #name>,)*
            }

            impl<'o> From<(peregrine::Duration, &'o peregrine::exec::SyncBump, #initial_conditions_name<'o>)> for #timelines_name<'o> {
                fn from((time, bump, inish_condish): (peregrine::Duration, &'o peregrine::exec::SyncBump, #initial_conditions_name)) -> Self {
                    Self {
                        #(#timeline_names: peregrine::timeline::Timeline::<#resource_paths, #name>::init(
                            time,
                            bump.alloc(peregrine::operation::InitialConditionOp::new(time, inish_condish.#resource_names))
                        ),)*
                    }
                }
            }

            #[derive(Default)]
            #visibility struct #histories_name<'h> {
                #(#history_names: <#resource_paths as peregrine::Resource<'h>>::History,)*
            }

            #(
                impl<'h> peregrine::history::HasHistory<'h, #resource_paths> for #histories_name<'h> {
                    fn insert(&'h self, hash: u64, value: <#resource_paths as peregrine::Resource<'h>>::Write) -> <#resource_paths as peregrine::Resource<'h>>::Read {
                        self.#history_names.insert(hash, value)
                    }
                    fn get(&'h self, hash: u64) -> Option<<#resource_paths as peregrine::Resource<'h>>::Read> {
                        self.#history_names.get(hash)
                    }
                }
            )*

            #(
                impl<'o> peregrine::timeline::HasTimeline<'o, #resource_paths, #name> for #timelines_name<'o> {
                    fn find_child(&self, time: peregrine::Duration) -> &'o (dyn peregrine::operation::Writer<'o, #resource_paths, #name>) {
                        let (last_time, last_op) = self.#timeline_names.last();
                        if last_time < time {
                            last_op
                        } else {
                            self.#timeline_names.last_before(time).1
                        }
                    }
                    fn insert_operation(&mut self, time: peregrine::Duration, op: &'o dyn peregrine::operation::Writer<'o, #resource_paths, #name>) -> &'o dyn peregrine::operation::Writer<'o, #resource_paths, #name> {
                        self.#timeline_names.insert(time, op)
                    }
                    fn remove_operation(&mut self, time: peregrine::Duration) {
                        self.#timeline_names.remove(time);
                    }

                    fn get_operations(&self, bounds: impl std::ops::RangeBounds<peregrine::Duration>) -> Vec<(peregrine::Duration, &'o dyn peregrine::operation::Writer<'o, #resource_paths, #name>)> {
                        self.#timeline_names.range(bounds).map(|(t,n)| (t, n)).collect()
                    }
                }
            )*
        };

        tokens.append_all(result);
    }
}
