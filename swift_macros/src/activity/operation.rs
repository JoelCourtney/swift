use std::collections::HashMap;

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

pub(crate) fn process_operation(input: String) -> TokenStream {
    let mut writes = HashMap::new();
    let mut read_writes = HashMap::new();

    let activity_start = input
        .find("activity")
        .expect("could not find activity label")
        + 8;
    let activity_end = input[activity_start..]
        .find(';')
        .expect("could not find activity end")
        + activity_start;

    let activity = format_ident!("{}", input[activity_start..activity_end].trim());

    let reads_start = input.find("reads").expect("could not find reads start") + 5;
    let reads_end = input[reads_start..]
        .find(';')
        .expect("could not find reads end")
        + reads_start;

    let temp_reads = input[reads_start..reads_end]
        .split(',')
        .map(|s| {
            let colon = s.find(':');
            match colon {
                None => panic!("no colon in read"),
                Some(c) => {
                    let name = format_ident!("{}", s[..c].trim());
                    let path: TokenStream = s[c + 1..]
                        .parse()
                        .expect("could not parse read resource type path");
                    (name, path)
                }
            }
        })
        .collect::<HashMap<_, _>>();

    let writes_start = input.find("writes").expect("could not find writes start") + 6;
    let writes_end = input[writes_start..]
        .find(";")
        .expect("could not find writes end")
        + writes_start;

    input[writes_start..writes_end].split(',').for_each(|s| {
        let colon = s.find(':');
        match colon {
            None => {
                let name = format_ident!("{}", s.trim());
                match temp_reads.get(&name) {
                    None => panic!("write variable doesn't have a resource type: {name}"),
                    Some(ty) => read_writes.insert(name, ty.clone()),
                };
            }
            Some(c) => {
                let name = format_ident!("{}", s[..c].trim());
                let path: TokenStream = s[c + 1..]
                    .parse()
                    .expect("could not parse write resource type path");
                writes.insert(name, path);
            }
        }
    });

    let reads = temp_reads
        .into_iter()
        .filter(|(n, _)| !read_writes.contains_key(n))
        .collect();

    let uuid = uuid::Uuid::new_v4().to_string().replace("-", "_");
    let op_inner = format_ident!("{activity}OpInner_{uuid}");
    let output_ident = format_ident!("{activity}OpOutput_{uuid}");
    let op = format_ident!("{activity}Op_{uuid}");

    let idents = Idents {
        op_inner,
        op,
        output: output_ident,
        activity,
        reads,
        writes,
        read_writes,
    };

    let when_start = input.find("when").expect("could not find when start") + 4;
    let when_end = input[when_start..]
        .find(';')
        .expect("could not find when end")
        + when_start;
    let when: TokenStream = input[when_start..when_end]
        .parse()
        .expect("could not parse when clause");

    let op_start = input.find("op").expect("could not find op start") + 2;
    let operation_body: TokenStream = input[op_start..]
        .to_string()
        .parse()
        .expect("could not parse op body");
    let op = generate_operation(&idents, operation_body);

    let output_struct = generate_output(&idents);

    let insert_into_plan = insert_into_plan(&idents, when);

    quote! {
        {
            #op
            #output_struct
            #insert_into_plan
        }
    }
}

struct Idents {
    op_inner: Ident,
    op: Ident,
    output: Ident,
    activity: Ident,
    reads: HashMap<Ident, TokenStream>,
    writes: HashMap<Ident, TokenStream>,
    read_writes: HashMap<Ident, TokenStream>,
}

fn generate_operation(idents: &Idents, body: TokenStream) -> TokenStream {
    let (read_only_variables, read_only_paths) = idents.reads.iter().collect::<(Vec<_>, Vec<_>)>();
    let (write_only_variables, write_only_paths) =
        idents.writes.iter().collect::<(Vec<_>, Vec<_>)>();
    let (read_write_variables, read_write_paths) =
        idents.read_writes.iter().collect::<(Vec<_>, Vec<_>)>();

    let all_paths = read_only_paths
        .iter()
        .chain(write_only_paths.iter())
        .chain(read_write_paths.iter())
        .collect::<Vec<_>>();

    let all_read_variables = read_only_variables
        .iter()
        .chain(read_write_variables.iter())
        .collect::<Vec<_>>();
    let all_write_variables = write_only_variables
        .iter()
        .chain(read_write_variables.iter())
        .collect::<Vec<_>>();

    let all_read_paths = read_only_paths
        .iter()
        .chain(read_write_paths.iter())
        .collect::<Vec<_>>();
    let all_write_paths = write_only_paths
        .iter()
        .chain(read_write_paths.iter())
        .collect::<Vec<_>>();

    let first_write_variable = all_write_variables[0];
    let all_but_one_write_variables = &all_write_variables[1..];

    let first_write_path = all_write_paths[0];
    let all_but_one_write_paths = &all_write_paths[1..];

    let read_only_resource_hashes = read_only_variables
        .iter()
        .map(|i| format_ident!("_swift_engine_resource_hash_{i}"))
        .collect::<Vec<_>>();
    let read_write_resource_hashes = read_write_variables
        .iter()
        .map(|i| format_ident!("_swift_engine_resource_hash_{i}"))
        .collect::<Vec<_>>();
    let all_read_resource_hashes = all_read_variables
        .iter()
        .map(|i| format_ident!("_swift_engine_resource_hash_{i}"))
        .collect::<Vec<_>>();

    let Idents {
        op_inner,
        op,
        output,
        activity,
        ..
    } = idents;

    let run_internal = quote! {
        let new_env = env.increment();

        #(let (#read_only_resource_hashes, #read_only_variables) = op_internal.#read_only_variables
                .read(histories, env)
                .await;
        )*
        #(let mut #write_only_variables = <#write_only_paths as swift::Resource<'o>>::Write::default();)*

        #(
            let (#read_write_resource_hashes, mut #read_write_variables): (u64, <#read_write_paths as swift::Resource<'o>>::Write) = {
                let (hash, #read_write_variables) = op_internal.#read_write_variables
                    .read(histories, env)
                    .await;
                (hash, (*#read_write_variables).into())
            };
        )*

        let hash = {
            use std::hash::{Hasher, BuildHasher, Hash};

            let mut state = swift::history::SwiftDefaultHashBuilder::default().build_hasher();
            std::any::TypeId::of::<#op_inner<swift::operation::AllModel>>().hash(&mut state);

            #(#all_read_resource_hashes.hash(&mut state);)*

            state.finish()
        };

        let (#(#all_write_variables),*) = if let Some(#first_write_variable) = <M::Histories as swift::HasHistory<#first_write_path>>::get(histories, hash) {
            #(let #all_but_one_write_variables = <M::Histories as swift::HasHistory<#all_but_one_write_paths>>::get(histories, hash).unwrap();)*
            (#(#all_write_variables),*)
        } else {
            { #body }
            #(let #all_write_variables = <M::Histories as swift::HasHistory<#all_write_paths>>::insert(histories, hash, #all_write_variables);)*
            (#(#all_write_variables),*)
        };

        #(drop(#read_only_variables);)*

        Some(#output {
            hash,
            #(#all_write_variables,)*
        })
    };

    let plan_bound = quote! {
        M::Plan: #(swift::HasResource<'o, #all_paths>)+*
    };

    let history_bound = quote! {
        M::Histories: #(swift::HasHistory<'o, #all_write_paths>)+*
    };

    quote! {
        struct #op_inner<'o, M: swift::Model<'o>> {
            #(#all_read_variables: &'o dyn swift::Writer<'o, #all_read_paths, M>,)*
            output: Option<#output<'o>>,
            parents: Vec<&'o dyn swift::Operation<'o, M>>
        }

        struct #op<'o, M: swift::Model<'o>> {
            inner: swift::reexports::tokio::sync::RwLock<#op_inner<'o, M>>,
            this: &'o #activity,
        }

        #[swift::reexports::async_trait::async_trait]
        impl<'o, M: swift::Model<'o>> swift::Operation<'o, M> for #op<'o, M>
        where #plan_bound {
            async fn find_children(&self, time: swift::Epoch, plan: &M::Plan) {
                let mut write = self.inner.write().await;
                #(
                    let new_child = <M::Plan as swift::HasResource<'o, #all_read_paths>>::find_child(plan, time);
                    if !std::ptr::eq(new_child, write.#all_read_variables) {
                        write.#all_read_variables.remove_parent(self).await;
                        write.#all_read_variables = new_child;
                    }
                )*
            }
            async fn add_parent(&self, parent: &'o dyn swift::Operation<'o, M>) {
                let mut write = self.inner.write().await;
                write.parents.push(parent);
            }
            async fn remove_parent(&self, parent: &dyn swift::Operation<'o, M>) {
                let mut write = self.inner.write().await;
                write.parents.retain(|p| !std::ptr::eq(*p, parent));
            }
        }

        #(
            impl<'o, M: swift::Model<'o>> swift::Writer<'o, #all_write_paths, M> for #op<'o, M>
            where #plan_bound, #history_bound {
                fn read<'b>(&'o self, histories: &'o M::Histories, env: swift::exec::ExecEnvironment<'b>) -> swift::exec::BumpedFuture<'b, (u64, swift::reexports::tokio::sync::RwLockReadGuard<'o, <#all_write_paths as swift::Resource<'o>>::Read>)> where 'o: 'b {
                    unsafe { std::pin::Pin::new_unchecked(env.bump.alloc(async move {
                        // If you (the thread) can get the write lock on the node, then you are responsible
                        // for calculating the hash and value if they aren't present.
                        // Otherwise, wait for a read lock and return the cached results.
                        let read: swift::reexports::tokio::sync::RwLockReadGuard<_> = if let Ok(mut write) = self.inner.try_write() {
                            if write.output.is_none() {
                                let result = if env.should_spawn == swift::exec::ShouldSpawn::Yes {
                                    let op_internal = &write;
                                    swift::exec::EXECUTOR.spawn_scoped(async move {
                                        let new_bump = swift::exec::SendBump::new();
                                        let env = swift::exec::ExecEnvironment::new(&new_bump);
                                        #run_internal
                                    }).await
                                } else {
                                    let op_internal = &write;
                                    #run_internal
                                };
                                write.output = result;
                                write.downgrade()
                            } else {
                                write.downgrade()
                            }
                        } else {
                            self.inner.read().await
                        };

                        (
                            read.output.as_ref().unwrap().hash,
                            swift::reexports::tokio::sync::RwLockReadGuard::map(read, |o| &o.output.as_ref().unwrap().#all_write_variables)
                        )
                    }))}
                }
            }
        )*
    }
}

fn generate_output(idents: &Idents) -> TokenStream {
    let (write_only_variables, write_only_paths) =
        idents.writes.iter().collect::<(Vec<_>, Vec<_>)>();
    let (read_write_variables, read_write_paths) =
        idents.read_writes.iter().collect::<(Vec<_>, Vec<_>)>();

    let all_write_variables = write_only_variables
        .iter()
        .chain(read_write_variables.iter())
        .collect::<Vec<_>>();

    let all_write_paths = write_only_paths
        .iter()
        .chain(read_write_paths.iter())
        .collect::<Vec<_>>();

    let Idents { output, .. } = idents;
    quote! {
        #[derive(Clone, Default)]
        struct #output<'h> {
            hash: u64,
            #(#all_write_variables: <#all_write_paths as swift::Resource<'h>>::Read,)*
        }
    }
}

fn insert_into_plan(idents: &Idents, when: TokenStream) -> TokenStream {
    let Idents { op, op_inner, .. } = idents;

    let (read_only_variables, read_only_paths) = idents.reads.iter().collect::<(Vec<_>, Vec<_>)>();
    let (_, write_only_paths) = idents.writes.iter().collect::<(Vec<_>, Vec<_>)>();
    let (read_write_variables, read_write_paths) =
        idents.read_writes.iter().collect::<(Vec<_>, Vec<_>)>();

    let all_read_variables = read_only_variables
        .iter()
        .chain(read_write_variables.iter())
        .collect::<Vec<_>>();

    let all_read_paths = read_only_paths
        .iter()
        .chain(read_write_paths.iter())
        .collect::<Vec<_>>();

    let all_write_paths = write_only_paths
        .iter()
        .chain(read_write_paths.iter())
        .collect::<Vec<_>>();

    quote! {
        {
            let when = #when;

            let op_inner = #op_inner {
                #(#all_read_variables: <M::Plan as swift::HasResource<#all_read_paths>>::find_child(plan, when),)*
                output: None,
                parents: vec![]
            };

            let op = bump.alloc(#op {
                inner: swift::reexports::tokio::sync::RwLock::new(op_inner),
                this: &self
            });

            #(<M::Plan as swift::HasResource<#all_write_paths>>::insert_operation(plan, when, op);)*
        }
    }
}
