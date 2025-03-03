use crate::operation::{Context, Op};
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{Expr, Path, Stmt};

mod input;
mod output;

pub fn process_activity(mut activity: Activity) -> TokenStream {
    let path = activity.path.clone();

    for line in &mut activity.lines {
        if let StmtOrInvoke::Invoke(Invocation {
            target: Target::Inline(op),
            ..
        }) = line
        {
            op.context = Context::Activity(path.clone());
        }
    }

    activity.into_token_stream()
}

#[derive(Debug)]
pub struct Activity {
    path: Path,
    _structure: ActivityStructure,
    lines: Vec<StmtOrInvoke>,
}

#[derive(Debug)]
pub enum ActivityStructure {
    Path,
    Item,
}

#[derive(Debug)]
enum StmtOrInvoke {
    Stmt(Stmt),
    Invoke(Invocation),
}

#[derive(Debug)]
struct Invocation {
    time: Placement,
    target: Target,
}

#[derive(Debug)]
struct Placement {
    start: Expr,
    delay: Option<Op>,
}

impl StmtOrInvoke {
    fn is_invoke(&self) -> bool {
        matches!(self, StmtOrInvoke::Invoke(..))
    }
}

#[derive(Debug)]
enum Target {
    Inline(Op),
    _Activity(Expr),
    _Routine(Expr),
}
