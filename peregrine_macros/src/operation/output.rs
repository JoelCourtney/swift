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
        let op_internals = format_ident!("{activity_ident}OpInternals_{uuid}");
        let op_body_function = format_ident!("{activity_ident}_op_body_{uuid}");
        let continuations = format_ident!("{activity_ident}Continuations_{uuid}");

        Idents {
            op_internals,
            op,
            output,
            continuations,
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
    op_internals: Ident,
    op: Ident,
    output: Ident,
    op_body_function: Ident,
    continuations: Ident,
    activity: Ident,
    write_onlys: Vec<Ident>,
    read_writes: Vec<Ident>,
    all_reads: Vec<Ident>,
    all_writes: Vec<Ident>,
}

fn generate_operation(idents: &Idents) -> TokenStream {
    let Idents {
        op_internals,
        op,
        output,
        op_body_function,
        continuations,
        activity,
        all_reads,
        all_writes,
        ..
    } = idents;

    let first_write = &all_writes[0];
    let all_but_one_write = &all_writes[1..];

    let all_read_response_hashes = all_reads
        .iter()
        .map(|i| format_ident!("_peregrine_engine_resource_hash_{i}"))
        .collect::<Vec<_>>();

    let all_read_responses = all_reads
        .iter()
        .map(|i| format_ident!("{i}_response"))
        .collect::<Vec<_>>();

    quote! {
        struct #op_internals<'o, M: peregrine::Model<'o>> {
            grounding_result: Option<peregrine::operation::InternalResult<peregrine::Duration>>,

            #(#all_reads: Option<&'o dyn peregrine::operation::Upstream<'o, #all_reads, M>>,)*
            #(#all_read_responses: Option<peregrine::operation::InternalResult<(u64, <#all_reads as peregrine::resource::Resource<'o>>::Read)>>,)*

            result: peregrine::operation::InternalResult<#output<'o>>
        }

        struct #op<'o, M: peregrine::Model<'o>> {
            grounding: peregrine::Grounding<'o, M>,
            grounding_state: peregrine::reexports::crossbeam::atomic::AtomicCell<peregrine::operation::OperationState>,
            value_state: peregrine::reexports::crossbeam::atomic::AtomicCell<peregrine::operation::OperationState>,
            response_counter: peregrine::reexports::crossbeam::atomic::AtomicCell<u8>,

            continuations: peregrine::reexports::parking_lot::Mutex<peregrine::operation::RecordedQueue<#continuations<'o, M>, #continuations<'o, M>>>,
            grounding_continuations: peregrine::reexports::parking_lot::Mutex<peregrine::operation::RecordedQueue<peregrine::operation::Continuation<'o, peregrine::operation::ungrounded::peregrine_grounding, M>, peregrine::operation::Continuation<'o, peregrine::operation::ungrounded::peregrine_grounding, M>>>,

            activity: &'o #activity,
            internals: peregrine::exec::UnsafeSyncCell<#op_internals<'o, M>>
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

        impl<'s, 'o: 's, M: peregrine::Model<'o>> #op<'o, M> {
            fn new(grounding: peregrine::Grounding<'o, M>, activity: &'o #activity) -> Self {
                #op {
                    grounding,
                    grounding_state: peregrine::reexports::crossbeam::atomic::AtomicCell::new(match grounding {
                        peregrine::Grounding::Static(t) => peregrine::operation::OperationState::Done,
                        _ => peregrine::operation::OperationState::Dormant,
                    }),
                    value_state: Default::default(),
                    response_counter: Default::default(),

                    continuations: Default::default(),
                    grounding_continuations: Default::default(),

                    activity,
                    internals: peregrine::exec::UnsafeSyncCell::new(#op_internals {
                        grounding_result: match grounding {
                            peregrine::Grounding::Static(t) => Some(Ok(t)),
                            _ => None
                        },

                        #(#all_reads: None,)*
                        #(#all_read_responses: None,)*

                        result: Err(peregrine::operation::ObservedErrorOutput)
                    }),
                }
            }
            fn run_value_continuations(&self, scope: &peregrine::reexports::rayon::Scope<'s>, timelines: &'s peregrine::timeline::Timelines<'o, M>, env: peregrine::exec::ExecEnvironment<'s, 'o>) {
                let result = unsafe {
                    (*self.internals.get()).result
                };
                let mut continuations = self.continuations.lock();

                let start_index = if env.stack_counter < peregrine::exec::STACK_LIMIT { 1 } else { 0 };

                let mut swapped_continuations = peregrine::reexports::smallvec::SmallVec::new();
                std::mem::swap(&mut continuations.new, &mut swapped_continuations);

                for c in swapped_continuations.drain(start_index..) {
                    match c {
                        #(#continuations::#all_writes(c) => {
                            if let Some(copy) = c.copy_node() {
                                continuations.old.push(#continuations::#all_writes(copy));
                            }
                            scope.spawn(move |s| c.run(result.map(|r| (r.hash, r.#all_writes)), s, timelines, env.reset()));
                        })*
                    }
                }

                if env.stack_counter < peregrine::exec::STACK_LIMIT {
                    match swapped_continuations.remove(0) {
                        #(#continuations::#all_writes(c) => {
                            if let Some(copy) = c.copy_node() {
                                continuations.old.push(#continuations::#all_writes(copy));
                            }
                            c.run(result.map(|r| (r.hash, r.#all_writes)), scope, timelines, env.increment());
                        })*
                    }
                }
            }

            fn run_grounding_continuations(&self, scope: &peregrine::reexports::rayon::Scope<'s>, timelines: &'s peregrine::timeline::Timelines<'o, M>, env: peregrine::exec::ExecEnvironment<'s, 'o>) {
                let grounding_result = unsafe {
                    (*self.internals.get()).grounding_result
                };
                let mut continuations = self.grounding_continuations.lock();

                assert!(!continuations.new.is_empty());
                let start_index = if env.stack_counter < peregrine::exec::STACK_LIMIT { 1 } else { 0 };

                let mut swapped_continuations = peregrine::reexports::smallvec::SmallVec::new();
                std::mem::swap(&mut continuations.new, &mut swapped_continuations);

                for c in swapped_continuations.drain(start_index..) {
                    if let Some(copy) = c.copy_node() {
                        continuations.old.push(copy);
                    }
                    scope.spawn(move |s| c.run(grounding_result.unwrap().map(|d| (0, d)), s, timelines, env.reset()));
                }

                if env.stack_counter < peregrine::exec::STACK_LIMIT {
                    let last = swapped_continuations.remove(0);
                    if let Some(copy) = last.copy_node() {
                        continuations.old.push(copy);
                    }
                    last.run(grounding_result.unwrap().map(|d| (0, d)), scope, timelines, env.increment());
                }
            }

            fn send_requests(&'o self, time: peregrine::Duration, scope: &peregrine::reexports::rayon::Scope<'s>, timelines: &'s peregrine::timeline::Timelines<'o, M>, env: peregrine::exec::ExecEnvironment<'s, 'o>) {
                let internals = self.internals.get();
                let (#(#all_read_responses,)*) = unsafe {
                    (#((*internals).#all_read_responses,)*)
                };
                let mut num_requests = 0u8
                    #(+ #all_read_responses.is_none() as u8)*;
                self.response_counter.store(num_requests);
                let time = unsafe {
                    (*internals).grounding_result.unwrap().unwrap()
                };
                #(
                    unsafe {
                        if (*internals).#all_reads.is_none() {
                            (*internals).#all_reads = Some(timelines.find_upstream(time))
                                .expect("Could not find an upstream node. Did you insert before the initial conditions?");
                        }
                    }
                    if #all_read_responses.is_none() {
                        num_requests -= 1;
                        let #all_reads = unsafe {
                            (*internals).#all_reads
                        };
                        if num_requests == 0 && env.stack_counter < peregrine::exec::STACK_LIMIT {
                            #all_reads.unwrap().request(peregrine::operation::Continuation::Node(self), scope, timelines, env.increment());
                        } else {
                            scope.spawn(move |s| #all_reads.unwrap().request(peregrine::operation::Continuation::Node(self), s, timelines, env.reset()));
                        }
                    }
                )*
            }

            fn run(&'o self, env: peregrine::exec::ExecEnvironment<'s, 'o>) -> peregrine::operation::InternalResult<#output<'o>> {
                use peregrine::Context;
                use peregrine::activity::ActivityLabel;

                let internals = self.internals.get();

                let (#((#all_read_response_hashes, #all_reads),)*) = unsafe {
                    (#((*internals).#all_read_responses.unwrap()?,)*)
                };

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
                    let time = unsafe {
                        (*self.internals.get()).grounding_result.unwrap().unwrap()
                    };
                    self.activity.#op_body_function(#(#all_reads,)*)
                        .with_context(|| format!("occurred in activity {} at {}", #activity::LABEL, time))
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

            fn clear_cached_continuations(&self) {
                use peregrine::operation::OperationState;

                match self.value_state.swap(OperationState::Dormant) {
                    OperationState::Dormant => {}
                    OperationState::Done => {
                        let mut continuations_lock = self.continuations.lock();
                        assert!(continuations_lock.new.is_empty());
                        for continuation in continuations_lock.old.drain(..) {
                            match continuation {
                                #(#continuations::#all_writes(c) => {
                                    match c {
                                        peregrine::operation::Continuation::Node(n) => n.clear_cache(),
                                        peregrine::operation::Continuation::MarkedNode(_, n) => n.clear_cache(),
                                        _ => unreachable!()
                                    }
                                })*
                            }
                        }
                    },
                    OperationState::Waiting => unreachable!()
                }
            }
        }

        impl<'o, M: peregrine::Model<'o>> peregrine::operation::Node<'o, M> for #op<'o, M> {
            fn insert_self(&'o self, timelines: &mut peregrine::timeline::Timelines<'o, M>, disruptive: bool) -> peregrine::Result<()> {
                let notify_time = self.grounding.min();
                #(
                    let previous = match self.grounding {
                        peregrine::Grounding::Static(t) => timelines.insert_grounded::<#all_writes>(t, self, disruptive),
                        peregrine::Grounding::Dynamic { min, max, .. } => timelines.insert_ungrounded::<#all_writes>(min, max, self, disruptive),
                    };
                    if disruptive {
                        assert!(previous.len() > 0);
                        for p in previous {
                            p.notify_downstreams(notify_time);
                        }
                    }
                )*
                Ok(())
            }
            fn remove_self(&self, timelines: &mut peregrine::timeline::Timelines<'o, M>) -> peregrine::Result<()> {
                #(
                    let removed = match self.grounding {
                        peregrine::Grounding::Static(t) => timelines.remove_grounded::<#all_writes>(t),
                        peregrine::Grounding::Dynamic { min, max, .. } => timelines.remove_ungrounded::<#all_writes>(min, max),
                    };
                    if !removed {
                        peregrine::bail!("Removal failed; could not find self at the expected time.")
                    }
                )*

                let mut lock = self.continuations.lock();
                assert!(lock.new.is_empty());
                for continuation in lock.old.drain(..) {
                    match continuation {
                        #(#continuations::#all_writes(c) => {
                            match c {
                                peregrine::operation::Continuation::Node(n) => n.clear_upstream(None),
                                peregrine::operation::Continuation::MarkedNode(_, n) => n.clear_upstream(None),
                                _ => unreachable!()
                            };
                        })*
                    }
                }

                Ok(())
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
                    debug_assert_eq!(OperationState::Waiting, self.value_state.load());

                    use peregrine::operation::OperationState;
                    use peregrine::activity::ActivityLabel;

                    unsafe {
                        (*self.internals.get()).#all_read_responses = Some(value);
                    }

                    if self.response_counter.fetch_sub(1) == 1 {
                        unsafe {
                            (*self.internals.get()).result = self.run(env);
                        }

                        // Its important that we set state to Done after the value is computed
                        // but BEFORE continuations are run, to prevent race condition.
                        // Better to have two tasks cooperatively working through the continuation queue
                        // with some contention than to accidentally leave a continuation due to race conditions.
                        self.value_state.store(OperationState::Done);

                        self.run_value_continuations(scope, timelines, env);
                    }
                }

                fn clear_cache(&self) {
                    unsafe {
                        (*self.internals.get()).#all_read_responses = None;
                    }
                    self.clear_cached_continuations();
                }

                fn clear_upstream(&self, time_of_change: Option<peregrine::Duration>) -> bool {
                    let internals = self.internals.get();
                    let (clear, retain) = if let Some(time_of_change) = time_of_change {
                        unsafe {
                            match (*internals).grounding_result {
                                Some(Ok(t)) if time_of_change < t => {
                                    (true, false)
                                }
                                Some(Ok(_)) => (false, true),
                                _ => (false, false)
                            }
                        }
                    } else { (true, false) };

                    if clear {
                        unsafe {
                            (*internals).#all_reads = None;
                            (*internals).#all_read_responses = None;
                        }
                        <Self as peregrine::operation::Downstream::<'o, #all_reads, M>>::clear_cache(self);
                    }

                    retain
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

                    match self.value_state.compare_exchange(OperationState::Dormant, OperationState::Waiting) {
                        Err(OperationState::Done) => {
                            self.run_value_continuations(scope, timelines, env);
                        }
                        Ok(OperationState::Dormant) => {
                            let internals = self.internals.get();
                            match self.grounding_state.compare_exchange(OperationState::Dormant, OperationState::Waiting) {
                                Err(OperationState::Done) => {
                                    unsafe {
                                        match (*internals).grounding_result.unwrap() {
                                            Ok(t) => self.send_requests(t, scope, timelines, env),
                                            Err(_) => self.run_value_continuations(scope, timelines, env)
                                        }
                                    }
                                }
                                Ok(OperationState::Dormant) => {
                                    self.grounding.unwrap_node().request(peregrine::operation::Continuation::Node(self), scope, timelines, env.increment());
                                }
                                Err(OperationState::Waiting) => {}
                                _ => unreachable!()
                            }
                        }
                        Err(OperationState::Waiting) => {}
                        _ => unreachable!()
                    }
                }

                fn notify_downstreams(&self, time_of_change: peregrine::Duration) {
                    let mut lock = self.continuations.lock();
                    assert!(lock.new.is_empty());
                    lock.old.retain(|continuation| {
                        match continuation {
                            #continuations::#all_writes(c) => {
                                match c {
                                    peregrine::operation::Continuation::Node(n) => n.clear_upstream(Some(time_of_change)),
                                    peregrine::operation::Continuation::MarkedNode(_, n) => n.clear_upstream(Some(time_of_change)),
                                    _ => unreachable!()
                                }
                            }
                            _ => true
                        }
                    })
                }
            }
        )*

        impl<'o, M: peregrine::Model<'o>> peregrine::operation::Upstream<'o, peregrine::operation::ungrounded::peregrine_grounding, M> for #op<'o, M> {
            fn request<'s>(
                &'o self,
                continuation: peregrine::operation::Continuation<'o, peregrine::operation::ungrounded::peregrine_grounding, M>,
                scope: &peregrine::reexports::rayon::Scope<'s>,
                timelines: &'s peregrine::timeline::Timelines<'o, M>,
                env: peregrine::exec::ExecEnvironment<'s, 'o>
            ) where 'o: 's {
                use peregrine::operation::OperationState;

                self.grounding_continuations.lock().new.push(continuation);

                match self.grounding_state.compare_exchange(OperationState::Dormant, OperationState::Waiting) {
                    Err(OperationState::Done) => {
                        self.run_grounding_continuations(scope, timelines, env);
                    }
                    Ok(OperationState::Dormant) => {
                        unsafe {
                            self.grounding.unwrap_node().request(peregrine::operation::Continuation::Node(self), scope, timelines, env.increment());
                        }
                    }
                    _ => {}
                }
            }

            fn notify_downstreams(&self, time_of_change: peregrine::Duration) {
                unreachable!()
            }
        }

        impl<'o, M: peregrine::Model<'o>> peregrine::operation::Downstream<'o, peregrine::operation::ungrounded::peregrine_grounding, M> for #op<'o, M> {
            fn respond<'s>(
                &'o self,
                value: peregrine::operation::InternalResult<(u64, peregrine::Duration)>,
                scope: &peregrine::reexports::rayon::Scope<'s>,
                timelines: &'s peregrine::timeline::Timelines<'o, M>,
                env: peregrine::exec::ExecEnvironment<'s, 'o>
            ) where 'o: 's {
                debug_assert_eq!(OperationState::Waiting, self.grounding_state.load());

                use peregrine::operation::OperationState;
                use peregrine::activity::ActivityLabel;

                self.grounding_state.store(OperationState::Done);

                self.run_grounding_continuations(scope, timelines, env);
                if value.is_err() {
                    self.run_value_continuations(scope, timelines, env);
                } else {
                    match self.value_state.load() {
                        OperationState::Waiting => self.send_requests(value.unwrap().1, scope, timelines, env),
                        OperationState::Dormant => {}
                        OperationState::Done => unreachable!()
                    }
                }
            }

            fn clear_cache(&self) {
                use peregrine::operation::OperationState;

                match self.grounding_state.swap(OperationState::Dormant) {
                    OperationState::Dormant => {}
                    OperationState::Done => {
                        let mut continuations_lock = self.grounding_continuations.lock();
                        assert!(continuations_lock.new.is_empty());
                        for continuation in continuations_lock.old.drain(..) {
                            match continuation {
                                peregrine::operation::Continuation::Node(n) => n.clear_cache(),
                                peregrine::operation::Continuation::MarkedNode(_, n) => n.clear_cache(),
                                _ => unreachable!()
                            }
                        }
                    },
                    OperationState::Waiting => unreachable!()
                }

                let internals = self.internals.get();
                unsafe {
                    #(
                        (*internals).#all_reads = None;
                        (*internals).#all_read_responses = None;
                    )*
                }

                self.clear_cached_continuations();
            }
            fn clear_upstream(&self, time_of_change: Option<peregrine::Duration>) -> bool {
                unreachable!()
            }
        }

        #(
            impl<'o, M: peregrine::Model<'o>> AsRef<dyn peregrine::operation::Upstream<'o, #all_writes, M> + 'o> for #op<'o, M> {
                fn as_ref(&self) -> &(dyn peregrine::operation::Upstream<'o, #all_writes, M> + 'o) {
                    self
                }
            }

            impl<'o, M: peregrine::Model<'o>> peregrine::operation::ungrounded::UngroundedUpstream<'o, #all_writes, M> for #op<'o, M> {}
        )*
    }
}

fn result(idents: &Idents) -> TokenStream {
    let Idents { op, .. } = idents;

    quote! {
        {
            |grounding: peregrine::Grounding<'o, M>, context, bump: peregrine::reexports::bumpalo_herd::Member<'o>| bump.alloc(#op::<'o, M>::new(grounding, context))
        }
    }
}
