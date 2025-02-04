use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use regex::{Captures, Regex};
use std::collections::HashSet;

pub(crate) fn process_operation(input: String) -> TokenStream {
    let comma_index = input.find(',').expect("no comma");
    let arrow_index = input.find("=>").expect("no arrow");
    let activity = input[0..comma_index].trim().to_string();
    let model = input[comma_index + 1..arrow_index].trim().to_string();
    let model_ident = format_ident!("{model}");

    let extras_module_ident = format_ident!("{model}_extras_module");

    let input = input[arrow_index + 2..].trim().to_string();

    let read_pat = Regex::new(r"(\?)([_a-zA-Z][_0-9a-zA-Z]*)").unwrap();
    let write_pat = Regex::new(r"(:)([_a-zA-Z][_0-9a-zA-Z]*)").unwrap();
    let read_write_pat = Regex::new(r"(\?:|:\?)([_a-zA-Z][_0-9a-zA-Z]*)").unwrap();
    let any_pat = Regex::new(r"(\?|:|\?:|:\?)([_a-zA-Z][_0-9a-zA-Z]*)").unwrap();

    let mut reads = HashSet::new();
    let mut writes = HashSet::new();

    for (_, [_, name]) in read_pat.captures_iter(&input).map(|c| c.extract()) {
        reads.insert(name);
    }

    for (_, [_, name]) in write_pat.captures_iter(&input).map(|c| c.extract()) {
        writes.insert(name);
    }

    for (_, [_, name]) in read_write_pat.captures_iter(&input).map(|c| c.extract()) {
        reads.insert(name);
        writes.insert(name);
    }

    let uuid = uuid::Uuid::new_v4().to_string().replace("-", "_");
    let bundle_ident = format_ident!("{activity}OpBundle_{uuid}");
    let op_ident = format_ident!("{activity}Op_{uuid}");
    let output_ident = format_ident!("{activity}OpOutput_{uuid}");

    let reads = reads.into_iter().collect::<Vec<_>>();
    let writes = writes.into_iter().collect::<Vec<_>>();

    let bundle = generate_bundle(
        bundle_ident.clone(),
        op_ident.clone(),
        model_ident.clone(),
        &reads,
        &writes,
    );

    let operation_body: TokenStream = any_pat
        .replace_all(&input, |caps: &Captures| {
            format!("_swift_engine_resource_{}", &caps[2])
        })
        .to_string()
        .parse()
        .expect("could not parse after replacing");
    let op = generate_operation(
        op_ident,
        output_ident.clone(),
        model_ident.clone(),
        extras_module_ident.clone(),
        &reads,
        &writes,
        operation_body,
    );

    let output_struct = generate_output(output_ident, extras_module_ident, &writes);

    quote! {
        {
            #bundle
            #op
            #output_struct
            #bundle_ident
        }
    }
}

fn generate_bundle(
    ident: Ident,
    op_ident: Ident,
    model_ident: Ident,
    reads: &Vec<&str>,
    writes: &Vec<&str>,
) -> TokenStream {
    let read_idents = reads
        .iter()
        .map(|r| format_ident!("{r}"))
        .collect::<Vec<_>>();
    let write_idents = writes
        .iter()
        .map(|r| format_ident!("{r}"))
        .collect::<Vec<_>>();

    let child_idents = reads
        .iter()
        .map(|r| format_ident!("_swift_internal_pls_no_touch_{r}_child"))
        .collect::<Vec<_>>();
    let write_node_idents = writes
        .iter()
        .map(|r| format_ident!("{r}_write_node"))
        .collect::<Vec<_>>();

    quote! {
        struct #ident;

        #[swift::reexports::async_trait::async_trait]
        impl OperationBundle<#model_ident> for #ident {
            async fn unpack(&self, time: swift::duration::Duration, timelines: &mut <#model_ident as swift::Model>::OperationTimelines) {
                #(let #child_idents = timelines.#read_idents.last_before(time);)*

                let op = std::sync::Arc::new(swift::reexports::tokio::sync::RwLock::new(#op_ident {
                    #(#child_idents: #child_idents.1.get_op(),)*
                    _swift_internal_pls_no_touch_output: None,
                }));

                #(let #write_node_idents = swift::operation::OperationNode::new(op.clone(), vec![]);)*

                #(timelines.#write_idents.insert(time, #write_node_idents);)*
            }
        }
    }
}

fn generate_operation(
    ident: Ident,
    output_ident: Ident,
    model_ident: Ident,
    extras_module_ident: Ident,
    reads: &Vec<&str>,
    writes: &Vec<&str>,
    body: TokenStream,
) -> TokenStream {
    let read_idents = reads
        .iter()
        .map(|r| format_ident!("{r}"))
        .collect::<Vec<_>>();
    let write_idents = writes
        .iter()
        .map(|r| format_ident!("{r}"))
        .collect::<Vec<_>>();

    let read_only_idents = read_idents
        .iter()
        .filter(|i| !write_idents.contains(i))
        .collect::<Vec<_>>();
    let write_only_idents = write_idents
        .iter()
        .filter(|i| !read_idents.contains(i))
        .collect::<Vec<_>>();
    let read_write_idents = read_idents
        .iter()
        .filter(|i| write_idents.contains(i))
        .collect::<Vec<_>>();

    let read_only_resource_idents = read_only_idents
        .iter()
        .map(|i| format_ident!("_swift_engine_resource_{i}"))
        .collect::<Vec<_>>();
    let write_only_resource_idents = write_only_idents
        .iter()
        .map(|i| format_ident!("_swift_engine_resource_{i}"))
        .collect::<Vec<_>>();
    let read_write_resource_idents = read_write_idents
        .iter()
        .map(|i| format_ident!("_swift_engine_resource_{i}"))
        .collect::<Vec<_>>();

    let all_write_resource_idents = write_idents
        .iter()
        .map(|i| format_ident!("_swift_engine_resource_{i}"))
        .collect::<Vec<_>>();

    let child_idents = reads
        .iter()
        .map(|r| format_ident!("_swift_internal_pls_no_touch_{r}_child"))
        .collect::<Vec<_>>();

    let read_only_child_idents = read_only_idents
        .iter()
        .map(|r| format_ident!("_swift_internal_pls_no_touch_{r}_child"))
        .collect::<Vec<_>>();
    let read_write_child_idents = read_write_idents
        .iter()
        .map(|r| format_ident!("_swift_internal_pls_no_touch_{r}_child"))
        .collect::<Vec<_>>();

    let child_resource_type_tag_idents = reads.iter().map(|r| format_ident!("{r}ResourceTypeTag"));
    let write_only_resource_type_tag_idents = write_only_idents
        .iter()
        .map(|i| format_ident!("{i}ResourceTypeTag"));
    let all_write_resource_type_tag_idents = write_idents
        .iter()
        .map(|i| format_ident!("{i}ResourceTypeTag"));

    quote! {
        struct #ident {
            #(#child_idents: std::sync::Arc<dyn swift::operation::Operation<#model_ident, crate::#extras_module_ident::#child_resource_type_tag_idents>>,)*
            _swift_internal_pls_no_touch_output: Option<#output_ident>
        }

        impl #ident {
            async fn run(&mut self, history: &<#model_ident as swift::Model>::History) {
                use swift::history::AsyncMap;

                #(let #read_only_resource_idents = *(self.#read_only_child_idents.run(history).await);)*
                #(let mut #write_only_resource_idents = <crate::#extras_module_ident::#write_only_resource_type_tag_idents as swift::resource::ResourceTypeTag>::ResourceType::default();)*
                #(let mut #read_write_resource_idents = *(self.#read_write_child_idents.run(history).await);)*

                #body

                self._swift_internal_pls_no_touch_output = Some(#output_ident {
                    #(#write_idents: #all_write_resource_idents.clone(),)*
                });

                let hash = self.history_hash();
                #(history.#write_idents.insert_async(hash, #all_write_resource_idents);)*
            }

            fn history_hash(&self) -> u64 {
                use std::hash::{Hash, BuildHasher, Hasher};

                let mut state = swift::history::SwiftDefaultHashBuilder::default().build_hasher();

                std::any::TypeId::of::<#ident>().hash(&mut state);

                #(self.#child_idents.history_hash().hash(&mut state);)*

                state.finish()
            }

            fn find_children(&mut self, time: swift::duration::Duration, timelines: &<#model_ident as swift::Model>::OperationTimelines) {
                #(self.#child_idents = timelines.#read_idents.last_before(time).1.get_op();)*
            }
        }

        #(
            #[swift::reexports::async_trait::async_trait]
            impl swift::operation::Operation<#model_ident, crate::#extras_module_ident::#all_write_resource_type_tag_idents> for swift::reexports::tokio::sync::RwLock<#ident> {
                async fn run(&self, history: &<#model_ident as swift::Model>::History) -> swift::reexports::tokio::sync::RwLockReadGuard<<crate::#extras_module_ident::#all_write_resource_type_tag_idents as swift::resource::ResourceTypeTag>::ResourceType> {
                    if let Ok(mut write) = self.try_write() {
                        write.run(history).await;
                    }

                    return swift::reexports::tokio::sync::RwLockReadGuard::map(self.read().await, |o| &o._swift_internal_pls_no_touch_output.as_ref().unwrap().#write_idents);
                }

                fn history_hash(&self) -> u64 {
                    self.try_read().unwrap().history_hash()
                }

                async fn find_children(&self, time: swift::duration::Duration, timelines: &<#model_ident as swift::Model>::OperationTimelines) {
                    self.write().await.find_children(time, timelines);
                }
            }
        )*
    }
}

fn generate_output(ident: Ident, extras_module_ident: Ident, writes: &Vec<&str>) -> TokenStream {
    let write_idents = writes
        .iter()
        .map(|r| format_ident!("{r}"))
        .collect::<Vec<_>>();
    let write_resource_type_tag_idents = writes.iter().map(|r| format_ident!("{r}ResourceTypeTag"));
    quote! {
        struct #ident {
            #(#write_idents: <crate::#extras_module_ident::#write_resource_type_tag_idents as swift::resource::ResourceTypeTag>::ResourceType,)*
        }
    }
}
