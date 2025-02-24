use crate::activity::{Activity, Op, StmtOrOp};
use proc_macro2::Ident;
use syn::parse::{Parse, ParseStream};
use syn::{Block, Error, Expr, Result, Stmt, Token, parenthesized};

impl Parse for Activity {
    fn parse(input: ParseStream) -> Result<Self> {
        <Token![for]>::parse(input)?;

        let name: Ident = input.parse()?;

        let mut lines: Vec<StmtOrOp> = vec![];
        while !input.is_empty() {
            lines.push(input.parse()?);
        }

        Ok(Activity { name, lines })
    }
}

impl Parse for StmtOrOp {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Token![@]) {
            Ok(StmtOrOp::Op(input.parse()?))
        } else {
            let forked = input.fork();
            let stmt: Result<Stmt> = forked.parse();
            if stmt.is_ok() {
                Ok(StmtOrOp::Stmt(input.parse()?))
            } else {
                let expr: Expr = input.parse()?;
                Ok(StmtOrOp::Stmt(Stmt::Expr(expr, None)))
            }
        }
    }
}

impl Parse for Op {
    fn parse(input: ParseStream) -> Result<Self> {
        <Token![@]>::parse(input)?;

        let when_buffer;
        parenthesized!(when_buffer in input);

        let when: Expr = when_buffer.parse()?;

        let mut reads_temp = vec![];
        let mut writes_temp = vec![];

        loop {
            let variable: Ident = input.parse()?;

            if reads_temp.contains(&variable) {
                return Err(Error::new_spanned(
                    variable,
                    "Identifier declared as read multiple times.",
                ));
            }

            reads_temp.push(variable);

            if input.peek(Token![,]) {
                <Token![,]>::parse(input)?;
            } else {
                <Token![->]>::parse(input)?;
                break;
            }
        }

        loop {
            let variable: Ident = input.parse()?;

            if writes_temp.contains(&variable) {
                return Err(Error::new_spanned(
                    variable,
                    "Identifier declared as write multiple times.",
                ));
            }

            writes_temp.push(variable);

            if input.peek(Token![,]) {
                <Token![,]>::parse(input)?;
            } else {
                break;
            }
        }

        let mut reads = vec![];
        let mut writes = vec![];
        let mut read_writes = vec![];

        for v in &reads_temp {
            if writes_temp.contains(v) {
                read_writes.push(v.clone());
            } else {
                reads.push(v.clone());
            }
        }

        for v in writes_temp {
            if !reads_temp.contains(&v) {
                writes.push(v);
            }
        }

        let body: Block = input.parse()?;

        Ok(Op {
            activity: None,
            reads,
            writes,
            read_writes,
            when,
            body,
            uuid: uuid::Uuid::new_v4().to_string().replace("-", "_"),
        })
    }
}
