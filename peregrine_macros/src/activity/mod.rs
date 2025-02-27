use proc_macro2::{Ident, TokenStream};
use quote::ToTokens;
use syn::{Block, Expr, Path, Stmt};

mod input;
mod operation;
mod output;

pub fn process_activity(mut activity: Activity) -> TokenStream {
    let name = activity.name.clone();

    for line in &mut activity.lines {
        if let StmtOrInvoke::Invoke(_, Target::Op(op)) = line {
            op.input = OpInput::Activity(name.clone());
        }
    }

    activity.into_token_stream()
}

pub struct Activity {
    name: Ident,
    lines: Vec<StmtOrInvoke>,
}

enum StmtOrInvoke {
    Stmt(Stmt),
    Invoke(When, Target),
}

struct When {
    start: Expr,
    delay: Expr,
}

impl StmtOrInvoke {
    fn is_invoke(&self) -> bool {
        matches!(self, StmtOrInvoke::Invoke(..))
    }
    fn get_invoke(&self) -> Option<(&When, &Target)> {
        match self {
            StmtOrInvoke::Stmt(_) => None,
            StmtOrInvoke::Invoke(when, target) => Some((when, target)),
        }
    }
}

enum Target {
    Op(Op),
    Activity(Expr),
    Routine(Expr),
}

struct Op {
    input: OpInput,
    reads: Vec<Ident>,
    writes: Vec<Ident>,
    read_writes: Vec<Ident>,
    body: Block,
    uuid: String,
}

enum OpInput {
    Activity(Ident),
    Routine(Ident),
    None,
}
