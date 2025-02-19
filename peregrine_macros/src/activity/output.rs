use crate::activity::operation::process_operation;
use crate::activity::{Activity, Op, StmtOrOp};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens, TokenStreamExt};

impl ToTokens for Activity {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Activity { name, lines } = &self;

        let mut resources_used = vec![];
        for line in &self.lines {
            if let StmtOrOp::Op(op) = line {
                resources_used.extend(op.reads.values().cloned());
                resources_used.extend(op.writes.values().cloned());
                resources_used.extend(op.read_writes.values().cloned());
            }
        }

        let result = quote! {
            impl<'o, M: peregrine::Model<'o> + 'o> peregrine::Activity<'o, M> for #name
            where M::Timelines: 'o + #(peregrine::timeline::HasTimeline<'o, #resources_used, M>)+*, M::Histories: #(peregrine::history::HasHistory<'o, #resources_used>)+* {
                fn decompose(&'o self, start: peregrine::Time, timelines: &mut M::Timelines, bump: &'o peregrine::exec::SyncBump) -> peregrine::Duration {
                    #(#lines)*
                }
            }
        };

        tokens.append_all(result);
    }
}

impl ToTokens for StmtOrOp {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            StmtOrOp::Stmt(s) => {
                s.to_tokens(tokens);
            }
            StmtOrOp::Op(op) => {
                op.to_tokens(tokens);
            }
        }
    }
}

impl ToTokens for Op {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Op {
            activity,
            reads,
            writes,
            read_writes,
            when,
            body: op,
        } = self;

        let activity = activity.clone().expect("activity name was not set");

        let read_variables = reads.keys().chain(read_writes.keys());
        let read_paths = reads.values().chain(read_writes.values());

        let write_variables = writes.keys().chain(read_writes.keys());
        let write_paths = writes.values().chain(read_writes.values());

        let input = quote! {
            activity #activity;
            reads #(#read_variables: #read_paths),*;
            writes #(#write_variables: #write_paths),*;
            when #when;
            op #op
        };
        let result = process_operation(input.to_string());
        tokens.append_all(result);
    }
}
