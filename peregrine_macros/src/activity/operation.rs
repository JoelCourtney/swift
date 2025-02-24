use crate::activity::Op;
use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, TokenStreamExt, format_ident, quote};
use syn::Expr;

impl Op {
    pub(crate) fn body_function(&self) -> TokenStream {
        let Idents {
            all_reads,
            all_writes,
            write_onlys,
            read_writes,
            op_body_function,
            ..
        } = self.make_idents();

        let body = &self.body;

        quote! {
            fn #op_body_function<'h>(&self, #(#all_reads: <#all_reads as peregrine::resource::Resource<'h>>::Read,)*) -> peregrine::Result<(#(<#all_writes as peregrine::resource::Resource<'h>>::Write,)*)> {
                #(let mut #write_onlys: <#write_onlys as peregrine::resource::Resource<'h>>::Write;)*
                #(let mut #read_writes: <#read_writes as peregrine::resource::Resource<'h>>::Write = #read_writes.into();)*
                #body
                Ok((#(#all_writes,)*))
            }
        }
    }

    fn make_idents(&self) -> Idents {
        let Op {
            activity,
            reads,
            writes,
            read_writes,
            uuid,
            ..
        } = self;

        let activity = activity.clone().expect("activity name was not set");

        let output = format_ident!("{activity}OpOutput_{uuid}");
        let op = format_ident!("{activity}Op_{uuid}");
        let op_relationships = format_ident!("{activity}OpRelationships_{uuid}");
        let op_body_function = format_ident!("{activity}_op_body_{uuid}");

        Idents {
            op_relationships,
            op,
            output,
            op_body_function,
            activity,
            read_onlys: reads.clone(),
            write_onlys: writes.clone(),
            read_writes: read_writes.clone(),
            all_reads: reads.iter().chain(read_writes.iter()).cloned().collect(),
            all_writes: writes.iter().chain(read_writes.iter()).cloned().collect(),
            all_resources: reads
                .iter()
                .chain(writes.iter())
                .chain(read_writes.iter())
                .cloned()
                .collect(),
        }
    }
}

impl ToTokens for Op {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let idents = self.make_idents();

        let result = process_operation(idents, &self.when);
        tokens.append_all(result);
    }
}

struct Idents {
    op_relationships: Ident,
    op: Ident,
    output: Ident,
    op_body_function: Ident,
    activity: Ident,
    read_onlys: Vec<Ident>,
    write_onlys: Vec<Ident>,
    read_writes: Vec<Ident>,
    all_reads: Vec<Ident>,
    all_writes: Vec<Ident>,
    all_resources: Vec<Ident>,
}

fn process_operation(idents: Idents, when: &Expr) -> TokenStream {
    let op = generate_operation(&idents);

    let output_struct = generate_output(&idents);

    let result = result(&idents, when);

    quote! {
        {
            #op
            #output_struct
            #result
        }
    }
}

fn generate_operation(idents: &Idents) -> TokenStream {
    let Idents {
        op_relationships,
        op,
        output,
        op_body_function,
        activity,
        read_onlys,
        all_reads,
        all_writes,
        all_resources,
        ..
    } = idents;

    let first_write = &all_writes[0];
    let all_but_one_write = &all_writes[1..];

    let all_read_resource_hashes = all_reads
        .iter()
        .map(|i| format_ident!("_peregrine_engine_resource_hash_{i}"))
        .collect::<Vec<_>>();

    let run_internal = quote! {
        let new_env = env.increment();

        let (#(#all_reads,)*) = {
            let relationships_lock = self.relationships.lock();
            (#(relationships_lock.#all_reads,)*)
        };

        #(
            let (#all_read_resource_hashes, #all_reads) = #all_reads
                .read(history, new_env)
                .await?;
        )*

        let hash = {
            use std::hash::{Hasher, BuildHasher, Hash};

            let mut state = peregrine::history::PeregrineDefaultHashBuilder::default().build_hasher();
            std::any::TypeId::of::<#output>().hash(&mut state);

            #(#all_read_resource_hashes.hash(&mut state);)*

            state.finish()
        };

        let (#(#all_writes),*) = if let Some(#first_write) = history.get::<#first_write>(hash) {
            #(let #all_but_one_write = history.get::<#all_but_one_write>(hash).expect("expected all write outputs from past run to be written to history");)*
            (#(#all_writes),*)
        } else {
            use peregrine::{Activity, Context};
            let (#(#all_writes,)*) = self.activity.#op_body_function(#(*#all_reads,)*)
                .with_context(|| format!("occured in activity {} at {}", self.activity.label(), self.time))?;
            #(let #all_writes = history.insert::<#all_writes>(hash, #all_writes);)*
            (#(#all_writes),*)
        };

        #(drop(#read_onlys);)*

        Ok::<_, peregrine::Error>(#output {
            hash,
            #(#all_writes,)*
        })
    };

    let timelines_bound = quote! {
        M::Timelines: #(peregrine::timeline::HasTimeline<'o, #all_resources, M>)+*
    };

    quote! {
        struct #op_relationships<'o, M: peregrine::Model<'o>> {
            parents: Vec<&'o dyn peregrine::operation::Operation<'o, M>>,
            #(#all_reads: &'o dyn peregrine::operation::Writer<'o, #all_reads, M>,)*

        }

        struct #op<'o, M: peregrine::Model<'o>> {
            result: peregrine::reexports::tokio::sync::RwLock<Option<Result<#output<'o>, peregrine::operation::ObservedErrorOutput>>>,
            relationships: peregrine::reexports::parking_lot::Mutex<#op_relationships<'o, M>>,
            activity: &'o #activity,
            time: peregrine::Duration,
        }

        impl<'o, M: peregrine::Model<'o>> peregrine::operation::Operation<'o, M> for #op<'o, M>
        where #timelines_bound {
            fn find_children(&'o self, time_of_change: peregrine::Duration, timelines: &M::Timelines) {
                if time_of_change >= self.time { return; }

                let mut changed = false;

                let mut lock = self.relationships.lock();
                #(
                    let new_child = <M::Timelines as peregrine::timeline::HasTimeline<'o, #all_reads, M>>::find_child(timelines, self.time).expect("unreachable; could not find a child that was previously there.");
                    if !std::ptr::eq(new_child, lock.#all_reads) {
                        lock.#all_reads.remove_parent(self);
                        new_child.add_parent(self);
                        lock.#all_reads = new_child;
                        changed = true;
                    }
                )*

                drop(lock);

                if changed {
                    let mut queue = std::collections::VecDeque::<&'o dyn peregrine::operation::Operation<'o, M>>::new();
                    queue.push_back(self);
                    while let Some(op) = queue.pop_front() {
                        if op.clear_cache() {
                            queue.extend(op.parents());
                        }
                    }
                }
            }
            fn add_parent(&self, parent: &'o dyn peregrine::operation::Operation<'o, M>) {
                self.relationships.lock().parents.push(parent);
            }
            fn remove_parent(&self, parent: &dyn peregrine::operation::Operation<'o, M>) {
                self.relationships.lock().parents.retain(|p| !std::ptr::eq(*p, parent));
            }

            fn insert_self(&'o self, timelines: &mut M::Timelines) -> peregrine::Result<()> {
                #(
                    let previous = <M::Timelines as peregrine::timeline::HasTimeline<#all_writes, M>>::insert_operation(timelines, self.time, self)
                        .ok_or_else(|| peregrine::anyhow!("Could not find an upstream node. Did you insert before the initial conditions?"))?;
                    previous.notify_parents(self.time, timelines);
                )*
                let lock = self.relationships.lock();
                #(lock.#all_reads.add_parent(self);)*
                Ok(())
            }
            fn remove_self(&self, timelines: &mut M::Timelines) -> peregrine::Result<()> {
                #(
                    let this = <M::Timelines as peregrine::timeline::HasTimeline<#all_writes, M>>::remove_operation(timelines, self.time);
                    if this.is_none() {
                        peregrine::bail!("Removal failed; could not find self at the expected time.")
                    }
                )*
                self.notify_parents(self.time, timelines);
                let lock = self.relationships.lock();
                #(lock.#all_reads.remove_parent(self);)*
                Ok(())
            }

            fn parents(&self) -> Vec<&'o dyn peregrine::operation::Operation<'o, M>> {
                self.relationships.lock().parents.clone()
            }
            fn notify_parents(&self, time_of_change: peregrine::Duration, timelines: &M::Timelines) {
                let lock = self.relationships.lock();
                let parents = lock.parents.clone();
                drop(lock);
                for parent in parents {
                    parent.find_children(time_of_change, timelines);
                }
            }
            fn clear_cache(&self) -> bool {
                self.result.blocking_write().take().is_some()
            }
        }

        #(
            impl<'o, M: peregrine::Model<'o>> peregrine::operation::Writer<'o, #all_writes, M> for #op<'o, M>
            where #timelines_bound {
                fn read<'b>(&'o self, history: &'o peregrine::History, env: peregrine::exec::ExecEnvironment<'b>) -> peregrine::exec::BumpedFuture<'b, peregrine::Result<(u64, peregrine::reexports::tokio::sync::RwLockReadGuard<'o, <#all_writes as peregrine::resource::Resource<'o>>::Read>)>> where 'o: 'b {
                    unsafe { std::pin::Pin::new_unchecked(env.bump.alloc(async move {
                        // If you (the thread) can get the write lock on the node, then you are responsible
                        // for calculating the hash and value if they aren't present.
                        // Otherwise, wait for a read lock and return the cached results.
                        let read: peregrine::reexports::tokio::sync::RwLockReadGuard<_> = if let Ok(mut write) = self.result.try_write() {
                            if write.is_none() {
                                use peregrine::ActivityLabel;
                                // Preemptively store an error result in the output.
                                // If the operation succeeds this will be overwritten.
                                // If the operation fails, this leaves us free to return
                                // the error at any time without having to worry about writing
                                // it down.
                                *write = Some(Err(peregrine::operation::ObservedErrorOutput(
                                    peregrine::Time::from_tai_duration(self.time), self.activity.label()
                                )));
                                let result = if env.should_spawn == peregrine::exec::ShouldSpawn::Yes {
                                    let mut scoped_output: peregrine::Result<_> = Err(peregrine::anyhow!(""));
                                    let output_ref = &mut scoped_output;

                                    let fut = async move {
                                        let new_bump = peregrine::exec::SyncBump::new();
                                        let mut env = peregrine::exec::ExecEnvironment::new(&new_bump);
                                        *output_ref = (async || { #run_internal })().await;
                                    };
                                    peregrine::reexports::async_scoped::TokioScope::scope_and_collect(|scope| {
                                        scope.spawn(fut);
                                    }).await;
                                    scoped_output?
                                } else {
                                    #run_internal?
                                };
                                *write = Some(Ok(result));
                                write.downgrade()
                            } else {
                                write.downgrade()
                            }
                        } else {
                            self.result.read().await
                        };

                        Ok((
                            read.expect("result was not written")?.hash,
                            peregrine::reexports::tokio::sync::RwLockReadGuard::map(read, |o| &o.as_ref().unwrap().as_ref().unwrap().#all_writes)
                        ))
                    }))}
                }
            }
        )*
    }
}

fn generate_output(idents: &Idents) -> TokenStream {
    let Idents {
        write_onlys,
        read_writes,
        ..
    } = idents;

    let all_writes = write_onlys
        .iter()
        .chain(read_writes.iter())
        .collect::<Vec<_>>();

    let Idents { output, .. } = idents;
    quote! {
        #[derive(Copy, Clone, Default)]
        struct #output<'h> {
            hash: u64,
            #(#all_writes: <#all_writes as peregrine::resource::Resource<'h>>::Read,)*
        }
    }
}

fn result(idents: &Idents, when: &Expr) -> TokenStream {
    let Idents {
        op,
        op_relationships,
        read_onlys,
        read_writes,
        ..
    } = idents;

    let all_reads = read_onlys
        .iter()
        .chain(read_writes.iter())
        .collect::<Vec<_>>();

    quote! {
        {
            let when = peregrine::timeline::epoch_to_duration(#when);

            let op = bump.alloc(#op {
                result: peregrine::reexports::tokio::sync::RwLock::new(None),
                activity: &self,
                relationships: peregrine::reexports::parking_lot::Mutex::new(#op_relationships {
                    parents: Vec::with_capacity(2),
                    #(#all_reads: <M::Timelines as peregrine::timeline::HasTimeline<#all_reads, M>>::find_child(timelines, when).ok_or_else(|| peregrine::anyhow!("Could not find upstream node. Did you insert before the initial conditions?"))?,)*
                }),
                time: when
            });

            operations.push(op);
        }
    }
}
