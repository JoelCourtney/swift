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

        let plan_name = format_ident!("{name}Plan");
        let initial_conditions_name = format_ident!("{name}InitialConditions");
        let histories_name = format_ident!("{name}Histories");

        let result = quote! {
            #visibility struct #name;

            impl<'o> swift::Model<'o> for #name {
                type Plan = #plan_name<'o>;
                type InitialConditions = #initial_conditions_name<'o>;
                type Histories = #histories_name<'o>;

                fn new_plan(time: swift::Time, initial_conditions: Self::InitialConditions, bump: &'o swift::exec::SyncBump) -> Self::Plan {
                    #plan_name {
                        activities: std::collections::HashMap::new(),
                        bump,
                        #(#timeline_names: swift::timeline::Timeline::<#resource_paths, #name>::init(
                            time,
                            bump.alloc(swift::operation::InitialConditionOp::new(initial_conditions.#resource_names))
                        ),)*
                        id_counter: 0
                    }
                }
            }

            #visibility struct #initial_conditions_name<'h> {
                #(#resource_names: <#resource_paths as swift::Resource<'h>>::Write,)*
            }

            #visibility struct #plan_name<'o> {
                activities: std::collections::HashMap<swift::ActivityId, (swift::Time, &'o dyn swift::Activity<'o, #name>)>,
                bump: &'o swift::exec::SyncBump,
                #(#timeline_names: swift::timeline::Timeline<'o, #resource_paths, #name>,)*
                id_counter: u32
            }

            #[derive(Default)]
            #visibility struct #histories_name<'h> {
                #(#history_names: <#resource_paths as swift::Resource<'h>>::History,)*
            }

            #(
                impl<'h> swift::history::HasHistory<'h, #resource_paths> for #histories_name<'h> {
                    fn insert(&'h self, hash: u64, value: <#resource_paths as swift::Resource<'h>>::Write) -> <#resource_paths as swift::Resource<'h>>::Read {
                        self.#history_names.insert(hash, value)
                    }
                    fn get(&'h self, hash: u64) -> Option<<#resource_paths as swift::Resource<'h>>::Read> {
                        self.#history_names.get(hash)
                    }
                }
            )*

            impl<'o> swift::Plan<'o> for #plan_name<'o> {

                type Model = #name;

                fn insert(&mut self, time: swift::Time, activity: impl swift::Activity<'o, #name> + 'o) -> swift::ActivityId {
                    let id = swift::ActivityId::new(self.id_counter);
                    self.id_counter += 1;
                    let activity = self.bump.alloc(activity);
                    self.activities.insert(id, (time, activity));
                    let activity = &self.activities.get(&id).unwrap().1;

                    activity.decompose(time, self, &self.bump);

                    id
                }
                fn remove(&self, _id: swift::ActivityId) {
                    todo!()
                }
            }

            #(
                impl<'o> swift::timeline::HasResource<'o, #resource_paths> for #plan_name<'o> {
                    fn find_child(&self, time: swift::Time) -> &'o (dyn swift::operation::Writer<'o, #resource_paths, Self::Model>) {
                        let (last_time, last_op) = self.#timeline_names.last();
                        if last_time < time {
                            last_op
                        } else {
                            self.#timeline_names.last_before(time).1
                        }
                    }
                    fn insert_operation(&mut self, time: swift::Time, op: &'o dyn swift::operation::Writer<'o, #resource_paths, Self::Model>) {
                        self.#timeline_names.insert(time, op);
                    }

                    fn get_operations(&self, bounds: impl std::ops::RangeBounds<swift::Time>) -> Vec<(swift::Time, &'o dyn swift::operation::Writer<'o, #resource_paths, Self::Model>)> {
                        self.#timeline_names.range(bounds).map(|(t,n)| (t, n)).collect()
                    }
                }
            )*
        };

        tokens.append_all(result);
    }
}
