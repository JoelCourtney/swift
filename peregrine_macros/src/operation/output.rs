use crate::operation::{Context, Op};
use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, format_ident, quote};

impl Op {
    pub fn body_function(&self) -> TokenStream {
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
            context,
            reads,
            writes,
            read_writes,
            uuid,
            ..
        } = self;

        let activity = if let Context::Activity(p) = context {
            p.clone()
        } else {
            todo!()
        };

        let activity_ident = activity.get_ident().unwrap();

        let output = format_ident!("{activity_ident}OpOutput_{uuid}");
        let op = format_ident!("{activity_ident}Op_{uuid}");
        let op_relationships = format_ident!("{activity_ident}OpRelationships_{uuid}");
        let op_body_function = format_ident!("{activity_ident}_op_body_{uuid}");
        let continuations = format_ident!("{activity_ident}Continuations_{uuid}");
        let downstreams_enum = format_ident!("{activity_ident}Downstreams_{uuid}");

        Idents {
            op_relationships,
            op,
            output,
            continuations,
            downstreams_enum,
            op_body_function,
            activity: activity_ident.clone(),
            write_onlys: writes.clone(),
            read_writes: read_writes.clone(),
            all_reads: reads.iter().chain(read_writes.iter()).cloned().collect(),
            all_writes: writes.iter().chain(read_writes.iter()).cloned().collect(),
        }
    }
}

impl ToTokens for Op {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let idents = self.make_idents();
        let definition = generate_operation(&idents);
        let instantiation = result(&idents);

        let result = quote! {
            {
                #definition
                #instantiation
            }
        };

        tokens.extend(result);
    }
}

struct Idents {
    op_relationships: Ident,
    op: Ident,
    output: Ident,
    op_body_function: Ident,
    continuations: Ident,
    downstreams_enum: Ident,
    activity: Ident,
    write_onlys: Vec<Ident>,
    read_writes: Vec<Ident>,
    all_reads: Vec<Ident>,
    all_writes: Vec<Ident>,
}

fn generate_operation(idents: &Idents) -> TokenStream {
    let Idents {
        op_relationships,
        op,
        output,
        op_body_function,
        continuations,
        downstreams_enum,
        activity,
        all_reads,
        all_writes,
        ..
    } = idents;

    let first_write = &all_writes[0];
    let all_but_one_write = &all_writes[1..];
    let first_read = &all_reads[0];
    let all_but_one_read = &all_reads[1..];

    let all_read_response_hashes = all_reads
        .iter()
        .map(|i| format_ident!("_peregrine_engine_resource_hash_{i}"))
        .collect::<Vec<_>>();

    let all_read_responses = all_reads
        .iter()
        .map(|i| format_ident!("{i}_response"))
        .collect::<Vec<_>>();

    let num_reads = all_reads.len() as u8;

    quote! {
        struct #op_relationships<'o, M: peregrine::Model<'o>> {
            #(#all_reads: Option<&'o dyn peregrine::operation::Upstream<'o, #all_reads, M>>,)*
            #(#all_read_responses: peregrine::operation::InternalResult<(u64, <#all_reads as peregrine::resource::Resource<'o>>::Read)>,)*
        }

        struct #op<'o, M: peregrine::Model<'o>> {
            state: peregrine::reexports::crossbeam::atomic::AtomicCell<peregrine::operation::OperationState>,
            result: peregrine::operation::UnsyncUnsafeCell<peregrine::operation::InternalResult<#output<'o>>>,
            relationships: peregrine::reexports::parking_lot::Mutex<#op_relationships<'o, M>>,
            activity: &'o #activity,
            time: peregrine::Duration,
            continuations: peregrine::reexports::parking_lot::Mutex<peregrine::operation::RecordedQueue<#continuations<'o, M>, #downstreams_enum<'o, M>>>,
            response_counter: peregrine::reexports::crossbeam::atomic::AtomicCell<u8>
        }

        #[derive(Copy, Clone, Default)]
        struct #output<'h> {
            hash: u64,
            #(#all_writes: <#all_writes as peregrine::resource::Resource<'h>>::Read,)*
        }

        #[allow(non_camel_case_types)]
        enum #continuations<'o, M: peregrine::Model<'o>> {
            #(#all_writes(peregrine::operation::Continuation<'o, #all_writes, M>),)*
        }

        enum #downstreams_enum<'o, M: peregrine::Model<'o>> {
            #(#all_writes(&'o dyn peregrine::operation::Downstream<'o, #all_writes, M>),)*
        }

        impl<'s, 'o: 's, M: peregrine::Model<'o>> #op<'o, M> {
            fn run_continuations(&self, scope: &peregrine::reexports::rayon::Scope<'s>, timelines: &'s peregrine::timeline::Timelines<'o, M>, env: peregrine::exec::ExecEnvironment<'s, 'o>) {
                let result = unsafe {
                    *self.result.get()
                };
                let mut continuations = self.continuations.lock();

                if !continuations.new.is_empty() {
                    let start_index = if env.stack_counter < peregrine::exec::STACK_LIMIT { 1 } else { 0 };

                    let mut swapped_continuations = peregrine::reexports::smallvec::SmallVec::new();
                    std::mem::swap(&mut continuations.new, &mut swapped_continuations);

                    for c in swapped_continuations.drain(start_index..) {
                        match c {
                            #(#continuations::#all_writes(c) => {
                                if let Some(d) = c.get_downstream() {
                                    continuations.old.push(#downstreams_enum::#all_writes(d));
                                }
                                scope.spawn(move |s| c.run(result.map(|r| (r.hash, r.#all_writes)), s, timelines, env.reset()));
                            })*
                        }
                    }

                    if env.stack_counter < peregrine::exec::STACK_LIMIT {
                        match swapped_continuations.remove(0) {
                            #(#continuations::#all_writes(c) => {
                                if let Some(d) = c.get_downstream() {
                                    continuations.old.push(#downstreams_enum::#all_writes(d));
                                }
                                c.run(result.map(|r| (r.hash, r.#all_writes)), scope, timelines, env.increment());
                            })*
                        }
                    }
                }
            }

            fn send_requests(&'o self, mut relationships: peregrine::reexports::parking_lot::MutexGuard<'o, #op_relationships<'o, M>>, scope: &peregrine::reexports::rayon::Scope<'s>, timelines: &'s peregrine::timeline::Timelines<'o, M>, env: peregrine::exec::ExecEnvironment<'s, 'o>) {
                #(
                    if relationships.#all_reads.is_none() {
                        relationships.#all_reads = Some(timelines.find_upstream(self.time))
                            .expect("Could not find an upstream node. Did you insert before the initial conditions?");
                    }
                )*
                let (#(#all_reads,)*) = (#(relationships.#all_reads,)*);
                drop(relationships);

                self.response_counter.store(#num_reads);
                #(scope.spawn(move |s| #all_but_one_read.unwrap().request(peregrine::operation::Continuation::Node(self), s, timelines, env.reset()));)*

                if env.stack_counter < peregrine::exec::STACK_LIMIT {
                    #first_read.unwrap().request(peregrine::operation::Continuation::Node(self), scope, timelines, env.increment());
                } else {
                    scope.spawn(move |s| #first_read.unwrap().request(peregrine::operation::Continuation::Node(self), s, timelines, env.increment()));
                }
            }

            fn run(&'o self, relationships_lock: peregrine::reexports::parking_lot::MutexGuard<'o, #op_relationships<'o, M>>, env: peregrine::exec::ExecEnvironment<'s, 'o>) -> peregrine::operation::InternalResult<#output<'o>> {
                use peregrine::Context;
                use peregrine::activity::ActivityLabel;

                let (#((#all_read_response_hashes, #all_reads),)*) = (#(relationships_lock.#all_read_responses?,)*);
                drop(relationships_lock);

                let hash = {
                    use std::hash::{Hasher, BuildHasher, Hash};

                    let mut state = peregrine::history::PeregrineDefaultHashBuilder::default().build_hasher();
                    std::any::TypeId::of::<#output>().hash(&mut state);

                    #(#all_read_response_hashes.hash(&mut state);)*

                    state.finish()
                };

                let result = if let Some(#first_write) = env.history.get::<#first_write>(hash) {
                    #(let #all_but_one_write = env.history.get::<#all_but_one_write>(hash).expect("expected all write outputs from past run to be written to history");)*
                    Ok(#output {
                        hash,
                        #(#all_writes),*
                    })
                } else {
                    use peregrine::{Activity, Context};
                    use peregrine::activity::ActivityLabel;
                    self.activity.#op_body_function(#(#all_reads,)*)
                        .with_context(|| format!("occurred in activity {} at {}", #activity::LABEL, self.time))
                        .map(|(#(#all_writes,)*)| #output {
                            hash,
                            #(#all_writes: env.history.insert::<#all_writes>(hash, #all_writes),)*
                        })
                };

                result.map_err(|e| {
                    env.errors.push(e);
                    peregrine::operation::ObservedErrorOutput
                })
            }
        }

        impl<'o, M: peregrine::Model<'o>> peregrine::operation::Node<'o, M> for #op<'o, M> {
            fn insert_self(&'o self, timelines: &mut peregrine::timeline::Timelines<'o, M>, disruptive: bool) -> peregrine::Result<()> {
                #(
                    let previous = timelines.insert_grounded::<#all_writes>(self.time, self, disruptive);
                    if disruptive {
                        assert!(previous.len() > 0);
                        for p in previous {
                            p.notify_downstreams(self.time);
                        }
                    }
                )*
                Ok(())
            }
            fn remove_self(&self, timelines: &mut peregrine::timeline::Timelines<'o, M>) -> peregrine::Result<()> {
                #(
                    let removed = timelines.remove_grounded::<#all_writes>(self.time);
                    if !removed {
                        peregrine::bail!("Removal failed; could not find self at the expected time.")
                    }
                )*

                let mut lock = self.continuations.lock();
                assert!(lock.new.is_empty());
                for downstream in lock.old.drain(..) {
                    match downstream {
                        #(#downstreams_enum::#all_writes(d) => {
                            d.clear_upstream(None);
                        })*
                    }
                }

                Ok(())
            }

            fn clear_cache(&self) {
                use peregrine::operation::OperationState;

                match self.state.swap(OperationState::Dormant) {
                    OperationState::Dormant => {}
                    OperationState::Done => {
                        let mut continuations_lock = self.continuations.lock();
                        assert!(continuations_lock.new.is_empty());
                        for downstream in continuations_lock.old.drain(..) {
                            match downstream {
                                #(#downstreams_enum::#all_writes(d) => d.clear_cache(),)*
                            }
                        }
                    },
                    OperationState::Waiting => unreachable!()
                }
            }

            fn notify_downstreams(&self, time_of_change: peregrine::Duration) {
                let mut lock = self.continuations.lock();
                assert!(lock.new.is_empty());
                lock.old.retain(|downstream| {
                    match downstream {
                        #(#downstreams_enum::#all_writes(d) => d.clear_upstream(Some(time_of_change)),)*
                    }
                })
            }
        }

        #(
            impl<'o, M: peregrine::Model<'o>> peregrine::operation::Downstream<'o, #all_reads, M> for #op<'o, M> {
                fn respond<'s>(
                    &'o self,
                    value: peregrine::operation::InternalResult<(u64, <#all_reads as peregrine::resource::Resource<'o>>::Read)>,
                    scope: &peregrine::reexports::rayon::Scope<'s>,
                    timelines: &'s peregrine::timeline::Timelines<'o, M>,
                    env: peregrine::exec::ExecEnvironment<'s, 'o>
                ) where 'o: 's {
                    debug_assert_eq!(OperationState::Waiting, self.state.load());

                    use peregrine::operation::OperationState;
                    use peregrine::activity::ActivityLabel;

                    let mut relationships_lock = self.relationships.lock();
                    relationships_lock.#all_read_responses = value;

                    if self.response_counter.fetch_sub(1) == 1 {
                        unsafe {
                            *self.result.get() = self.run(relationships_lock, env);
                        }

                        // Its important that we set state to Done after the value is computed
                        // but BEFORE continuations are run, to prevent race condition.
                        // Better to have two tasks cooperatively working through the continuation queue
                        // with some contention than to accidentally leave a continuation due to race conditions.
                        self.state.store(OperationState::Done);

                        self.run_continuations(scope, timelines, env);
                    }
                }

                fn clear_upstream(&self, time_of_change: Option<peregrine::Duration>) -> bool {
                    use peregrine::operation::Node;

                    if time_of_change.map(|d| d < self.time).unwrap_or(true) {
                        let mut relationships = self.relationships.lock();
                        relationships.#all_reads = None;
                        self.clear_cache();
                        true
                    } else {
                        false
                    }
                }
            }
        )*

        #(
            impl<'o, M: peregrine::Model<'o>> peregrine::operation::Upstream<'o, #all_writes, M> for #op<'o, M> {
                fn request<'s>(
                    &'o self,
                    continuation: peregrine::operation::Continuation<'o, #all_writes, M>,
                    scope: &peregrine::reexports::rayon::Scope<'s>,
                    timelines: &'s peregrine::timeline::Timelines<'o, M>,
                    env: peregrine::exec::ExecEnvironment<'s, 'o>
                ) where 'o: 's {
                    use peregrine::operation::OperationState;

                    self.continuations.lock().new.push(#continuations::#all_writes(continuation));

                    match self.state.load() {
                        OperationState::Done => {
                            self.run_continuations(scope, timelines, env);
                        }
                        OperationState::Dormant => {
                            if let Some(relationships) = self.relationships.try_lock() {
                                self.state.store(OperationState::Waiting);
                                self.send_requests(relationships, scope, timelines, env);
                            }
                        }
                        OperationState::Waiting => {}
                    }
                }
            }
        )*
    }
}

fn result(idents: &Idents) -> TokenStream {
    let Idents {
        op,
        op_relationships,
        all_reads,
        ..
    } = idents;

    let all_read_responses = all_reads
        .iter()
        .map(|i| format_ident!("{i}_response"))
        .collect::<Vec<_>>();

    quote! {
        {
            |time| #op {
                state: peregrine::reexports::crossbeam::atomic::AtomicCell::new(peregrine::operation::OperationState::Dormant),
                result: peregrine::operation::UnsyncUnsafeCell::new(Err(peregrine::operation::ObservedErrorOutput)),
                continuations: peregrine::reexports::parking_lot::Mutex::new(peregrine::operation::RecordedQueue::new()),
                response_counter: peregrine::reexports::crossbeam::atomic::AtomicCell::new(0),
                activity: &self,
                relationships: peregrine::reexports::parking_lot::Mutex::new(#op_relationships {
                    #(#all_reads: None,)*
                    #(#all_read_responses: Err(peregrine::operation::ObservedErrorOutput),)*
                }),
                time,
            }
        }
    }
}
