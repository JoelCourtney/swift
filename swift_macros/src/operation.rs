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
    let activity_ident = format_ident!("{activity}");
    let bundle_ident = format_ident!("{activity}OpBundle_{uuid}");
    let op_ident = format_ident!("{activity}Op_{uuid}");
    let output_ident = format_ident!("{activity}OpOutput_{uuid}");

    let reads = reads
        .into_iter()
        .map(|s| format_ident!("{s}"))
        .collect::<Vec<_>>();
    let writes = writes
        .into_iter()
        .map(|s| format_ident!("{s}"))
        .collect::<Vec<_>>();

    let idents = Idents {
        activity: activity_ident,
        op: op_ident,
        bundle: bundle_ident.clone(),
        output: output_ident,
        model: model_ident,
        extras: extras_module_ident,
        reads,
        writes,
    };

    let bundle = generate_bundle(&idents);

    let operation_body: TokenStream = any_pat
        .replace_all(&input, |caps: &Captures| {
            format!("_swift_engine_resource_{}", &caps[2])
        })
        .to_string()
        .parse()
        .expect("could not parse after replacing");
    let op = generate_operation(&idents, operation_body);

    let output_struct = generate_output(&idents);

    quote! {
        {
            #bundle
            #op
            #output_struct
            #bundle_ident(_self_arc.clone())
        }
    }
}

struct Idents {
    activity: Ident,
    op: Ident,
    bundle: Ident,
    output: Ident,
    model: Ident,
    extras: Ident,
    reads: Vec<Ident>,
    writes: Vec<Ident>,
}

fn generate_bundle(idents: &Idents) -> TokenStream {
    let read_idents = &idents.reads;
    let write_idents = &idents.writes;

    let child_idents = idents
        .reads
        .iter()
        .map(|r| format_ident!("_swift_internal_pls_no_touch_{r}_child"))
        .collect::<Vec<_>>();
    let write_node_idents = idents
        .writes
        .iter()
        .map(|r| format_ident!("{r}_write_node"))
        .collect::<Vec<_>>();

    let Idents {
        bundle,
        activity,
        model,
        op,
        ..
    } = &idents;

    quote! {
        struct #bundle(std::sync::Arc<#activity>);

        #[swift::reexports::async_trait::async_trait]
        impl swift::operation::OperationBundle<#model> for #bundle {
            async fn unpack(&self, time: swift::duration::Duration, timelines: &mut <#model as swift::Model>::OperationTimelines, history: std::sync::Arc<<#model as swift::Model>::History>) {
                #(let #child_idents = timelines.#read_idents.last_before(time);)*

                let op = std::sync::Arc::new(swift::reexports::tokio::sync::RwLock::new(#op {
                    #(#child_idents: #child_idents.1.get_op_weak(),)*
                    _swift_internal_pls_no_touch_args: self.0.clone(),
                    _swift_internal_pls_no_touch_result: None,
                    _swift_internal_pls_no_touch_history: history.clone()
                }));

                #(let #write_node_idents = swift::operation::OperationNode::new(op.clone(), vec![]);)*

                #(timelines.#write_idents.insert(time, #write_node_idents);)*
            }
        }
    }
}

fn generate_operation(idents: &Idents, body: TokenStream) -> TokenStream {
    let read_idents = &idents.reads;
    let write_idents = &idents.writes;

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

    let read_only_resource_hashes = read_only_idents
        .iter()
        .map(|i| format_ident!("_swift_engine_resource_hash_{i}"))
        .collect::<Vec<_>>();
    let read_write_resource_hashes = read_write_idents
        .iter()
        .map(|i| format_ident!("_swift_engine_resource_hash_{i}"))
        .collect::<Vec<_>>();
    let all_read_resource_hashes = idents
        .reads
        .iter()
        .map(|i| format_ident!("_swift_engine_resource_hash_{i}"))
        .collect::<Vec<_>>();

    let all_write_resource_idents = write_idents
        .iter()
        .map(|i| format_ident!("_swift_engine_resource_{i}"))
        .collect::<Vec<_>>();

    let child_idents = idents
        .reads
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

    let child_resource_type_tag_idents = idents
        .reads
        .iter()
        .map(|r| format_ident!("{r}ResourceTypeTag"));
    let write_only_resource_type_tag_idents = write_only_idents
        .iter()
        .map(|i| format_ident!("{i}ResourceTypeTag"));
    let all_write_resource_type_tag_idents = write_idents
        .iter()
        .map(|i| format_ident!("{i}ResourceTypeTag"));

    let all_but_one_write_idents = &idents.writes[1..];
    let first_write_ident = &idents.writes[0];

    let Idents {
        op,
        extras,
        model,
        activity,
        output,
        ..
    } = idents;

    let run_internal = quote! {
        let history = &op_internal._swift_internal_pls_no_touch_history;
        let args = &*op_internal._swift_internal_pls_no_touch_args;

        let children_should_spawn = should_spawn.increment();

        #(let #read_only_child_idents = op_internal.#read_only_child_idents.upgrade().unwrap();)*
        #(let #read_write_child_idents = op_internal.#read_write_child_idents.upgrade().unwrap();)*

        #(let (#read_only_resource_hashes, #read_only_resource_idents) = #read_only_child_idents
                .run(children_should_spawn, b)
                .await;
        )*
        #(let mut #write_only_resource_idents = <crate::#extras::#write_only_resource_type_tag_idents as swift::resource::ResourceTypeTag>::ResourceType::default();)*

        #(
            let (#read_write_resource_hashes, mut #read_write_resource_idents) = {
                let (hash, #read_write_child_idents) = #read_write_child_idents
                    .run(children_should_spawn, b)
                    .await;
                (hash, #read_write_child_idents.clone())
            };
        )*

        let hash = {
            use std::hash::{Hasher, BuildHasher, Hash};

            let mut state = swift::history::SwiftDefaultHashBuilder::default().build_hasher();
            std::any::TypeId::of::<#op>().hash(&mut state);

            #(#all_read_resource_hashes.hash(&mut state);)*

            state.finish()
        };

        let (#(#write_idents),*) = if let Some(#first_write_ident) = history.#first_write_ident.get_async(hash) {
            #(let #all_but_one_write_idents = history.#all_but_one_write_idents.get_async(hash).unwrap();)*
            (#(#write_idents),*)
        } else {
            #body
            #(history.#write_idents.insert_async(hash, #all_write_resource_idents.clone());)*
            (#(#all_write_resource_idents),*)
        };

        #(drop(#read_only_resource_idents);)*

        Some((
            hash,
            #output {
                #(#write_idents,)*
            }
        ))
    };

    quote! {
        #[derive(Clone)]
        struct #op {
            #(#child_idents: std::sync::Weak<dyn swift::operation::Operation<#model, crate::#extras::#child_resource_type_tag_idents>>,)*
            _swift_internal_pls_no_touch_args: std::sync::Arc<#activity>,
            _swift_internal_pls_no_touch_history: std::sync::Arc<<#model as swift::Model>::History>,
            _swift_internal_pls_no_touch_result: Option<(u64, #output)>
        }

        impl #op {
            fn find_children(&mut self, time: swift::duration::Duration, timelines: &<#model as swift::Model>::OperationTimelines) {
                #(self.#child_idents = timelines.#read_idents.last_before(time).1.get_op_weak();)*
            }
        }

        #(
            impl swift::operation::Operation<#model, crate::#extras::#all_write_resource_type_tag_idents> for swift::reexports::tokio::sync::RwLock<#op> {
                fn run<'a>(&'a self, should_spawn: swift::operation::ShouldSpawn, b: &'a swift::alloc::SendBump) -> swift::alloc::BumpedFuture<'a, (u64, swift::reexports::tokio::sync::RwLockReadGuard<<crate::#extras::#all_write_resource_type_tag_idents as swift::resource::ResourceTypeTag>::ResourceType>)> {
                    unsafe { std::pin::Pin::new_unchecked(b.alloc(async move {
                        use swift::history::AsyncMap;
                        // If you (the thread) can get the write lock on the node, then you are responsible
                        // for calculating the hash and value if they aren't present.
                        // Otherwise, wait for a read lock and return the cached results.
                        let read = if let Ok(mut write) = self.try_write() {
                            if write._swift_internal_pls_no_touch_result.is_none() {
                                let result = if should_spawn == swift::operation::ShouldSpawn::Yes {
                                    let op_internal = write.clone();
                                    swift::reexports::tokio::task::spawn(async move {
                                        let new_bump = swift::alloc::SendBump::new();
                                        let b = &new_bump;
                                        #run_internal
                                    }).await.unwrap()
                                } else {
                                    let op_internal = &write;
                                    #run_internal
                                };
                                write._swift_internal_pls_no_touch_result = result;
                                write.downgrade()
                            } else {
                                write.downgrade()
                            }
                        } else {
                            self.read().await
                        };

                        (
                            read._swift_internal_pls_no_touch_result.as_ref().unwrap().0,
                            swift::reexports::tokio::sync::RwLockReadGuard::map(read, |o| &o._swift_internal_pls_no_touch_result.as_ref().unwrap().1.#write_idents)
                        )
                    }))}
                }

                fn find_children<'a>(&'a self, time: swift::duration::Duration, timelines: &'a <#model as swift::Model>::OperationTimelines, b: &'a swift::alloc::SendBump) -> swift::alloc::BumpedFuture<'a, ()> {
                    unsafe { std::pin::Pin::new_unchecked(b.alloc(async move {
                        self.write().await.find_children(time, timelines);
                    }))}
                }
            }
        )*
    }
}

fn generate_output(idents: &Idents) -> TokenStream {
    let write_idents = &idents.writes;
    let write_resource_type_tag_idents = idents
        .writes
        .iter()
        .map(|r| format_ident!("{r}ResourceTypeTag"));
    let Idents { output, extras, .. } = idents;
    quote! {
        #[derive(Clone)]
        struct #output {
            #(#write_idents: <crate::#extras::#write_resource_type_tag_idents as swift::resource::ResourceTypeTag>::ResourceType,)*
        }
    }
}
