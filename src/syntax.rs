// This file is part of the No# (Nosharp) programming language and is licensed under MIT License; see LICENSE.txt for details

use chumsky::{prelude::*, primitive::Select};
use logos::Logos;

#[derive(Debug, Clone)]
pub struct ParseDiagnostic {
    pub message: String,
    pub start: usize,
    pub end: usize,
    pub help: Option<String>,
    pub rendered: Option<String>,
}

pub fn render_diagnostic(
    source: &str,
    message: &str,
    start: usize,
    end: usize,
    label: &str,
    help: Option<&str>,
) -> Option<String> {
    use ariadne::{Color, Config, IndexType, Label, Report, ReportKind, Source};

    let span = start..end.max(start + 1);
    let mut report = Report::build(ReportKind::Error, ("main.no", span.clone()))
        .with_config(Config::default().with_index_type(IndexType::Byte))
        .with_message(message)
        .with_label(
            Label::new(("main.no", span))
                .with_message(label)
                .with_color(Color::Red),
        );

    if let Some(help) = help {
        report = report.with_help(help);
    }

    let mut rendered = Vec::new();
    report
        .finish()
        .write(("main.no", Source::from(source)), &mut rendered)
        .ok()?;

    String::from_utf8(rendered).ok()
}

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\n\f]+")]
#[logos(skip(r"//[^\r\n]*", allow_greedy = true))]
pub enum Token {
    #[token("string")]
    StringType,
    #[token("int")]
    IntType,
    #[token("float")]
    FloatType,
    #[token("bool")]
    BoolType,
    #[token("void")]
    VoidType,

    #[token("true")]
    True,
    #[token("false")]
    False,

    #[token("auto")]
    Auto,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("elseif")]
    Elseif,
    #[token("and")]
    And,
    #[token("or")]
    Or,
    #[token("not")]
    Not,
    #[token("return")]
    Return,
    #[token("for")]
    For,

    #[token("=")]
    Assign,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("==")]
    Equal,
    #[token("!=")]
    NotEqual,
    #[token(">=")]
    MoreOrEqual,
    #[token("<=")]
    LessOrEqual,
    #[token(">")]
    MoreThan,
    #[token("<")]
    LessThan,
    #[token("=>")]
    FatArrow,
    #[token("->")]
    ThinArrow,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token(",")]
    Comma,
    #[token("..")]
    Concat,

    #[regex("[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f32>().ok())]
    Float(f32),

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i32>().ok())]
    Int(i32),

    #[regex(r#""[^"]*""#, |lex| {
        let slice = lex.slice();
        slice[1..slice.len() - 1].to_string()
    })]
    String(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Equal,
    NotEqual,
    LessOrEqual,
    MoreOrEqual,
    LessThan,
    MoreThan,
    Concat,
    And,
    Or,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i32),
    Float(f32),
    String(String),
    Var(String),
    Bool(bool),

    Binary {
        op: Op,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },

    Unary {
        op: Op,
        operand: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        var_type: Option<String>,
        name: String,
        value: Expr,
    },

    Assign {
        name: String,
        value: Expr,
    },

    Function {
        return_type: String,
        name: String,
        params: Vec<(String, String)>, // (type, name)
        body: Vec<Stmt>,
    },

    If {
        condition: Expr,
        then_block: Vec<Stmt>,
        else_block: Option<Vec<Stmt>>,
    },

    For {
        var_type: String,
        name: String,
        start: Expr,
        end: Expr,
        step: Expr,
        body: Vec<Stmt>,
    },

    Expr(Expr),
    Return(Option<Expr>),
}

pub fn parser<'a>() -> impl Parser<'a, &'a [Token], Vec<Stmt>, extra::Err<Rich<'a, Token>>> {
    recursive(|stmt| {
        let ident = select! {
            Token::Ident(s) => s,
        };

        let type_parser = select! {
            Token::IntType => "int".to_string(),
            Token::FloatType => "float".to_string(),
            Token::StringType => "string".to_string(),
            Token::BoolType => "bool".to_string(),
            Token::VoidType => "void".to_string(),
            Token::Ident(s) => s,
        };

        let block = stmt
            .clone()
            .repeated()
            .collect()
            .delimited_by(just(Token::LBrace), just(Token::RBrace));

        let expr =
            recursive(|expr| {
                let int_lit = select! { Token::Int(n) => Expr::Int(n) };
                let float_lit = select! { Token::Float(f) => Expr::Float(f) };
                let str_lit = select! { Token::String(s) => Expr::String(s) };
                let ident_lit = select! { Token::Ident(s) => Expr::Var(s) };
                let bool_lit: Select<_, &'a [Token], Expr, extra::Err<Rich<'a, Token>>> = select! {
                    Token::True => Expr::Bool(true),
                    Token::False => Expr::Bool(false),
                };

                let args = expr
                    .clone()
                    .separated_by(just(Token::Comma))
                    .collect::<Vec<_>>();

                let atom = int_lit
                    .or(float_lit)
                    .or(str_lit)
                    .or(bool_lit)
                    .or(ident_lit)
                    .or(expr
                        .clone()
                        .delimited_by(just(Token::LParen), just(Token::RParen)));

                let call_expr = atom.clone().foldl(
                    args.delimited_by(just(Token::LParen), just(Token::RParen))
                        .repeated(),
                    |callee, args| Expr::Call {
                        callee: Box::new(callee),
                        args,
                    },
                );

                let math_op = select! {
                    Token::Plus => Op::Add,
                    Token::Minus => Op::Sub,
                    Token::Star => Op::Mul,
                    Token::Slash => Op::Div,
                };

                let math_expr = call_expr.clone().foldl(
                    math_op.then(call_expr.clone()).repeated(),
                    |left, (op, right)| Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                );

                let cmp_op = select! {
                    Token::Equal => Op::Equal,
                    Token::NotEqual => Op::NotEqual,
                    Token::LessOrEqual => Op::LessOrEqual,
                    Token::MoreOrEqual => Op::MoreOrEqual,
                    Token::LessThan => Op::LessThan,
                    Token::MoreThan => Op::MoreThan,
                    Token::Concat => Op::Concat
                };

                let cmp_expr = math_expr.clone().foldl(
                    cmp_op.then(math_expr).repeated(),
                    |left, (op, right)| Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                );

                let logic_op = select! {
                    Token::And => Op::And,
                    Token::Or => Op::Or,
                    Token::Not => Op::Not
                };

                let unary_expr = just(Token::Not)
                    .ignore_then(cmp_expr.clone())
                    .map(|operand| Expr::Unary {
                        op: Op::Not,
                        operand: Box::new(operand),
                    })
                    .or(cmp_expr.clone());

                let logic_expr = unary_expr.clone().foldl(
                    logic_op.then(unary_expr).repeated(),
                    |left, (op, right)| Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                );

                return logic_expr;
            });

        let typed_let_stmt = type_parser
            .clone()
            .then(ident.clone())
            .then_ignore(just(Token::Assign))
            .then(expr.clone())
            .map(|((var_type, name), value)| Stmt::Let {
                var_type: Some(var_type),
                name,
                value,
            });

        let let_stmt = just(Token::Auto)
            .ignore_then(ident.clone())
            .then_ignore(just(Token::Assign))
            .then(expr.clone())
            .map(|(name, value)| Stmt::Let {
                var_type: None,
                name,
                value,
            });

        let assign_stmt = ident
            .clone()
            .then_ignore(just(Token::Assign))
            .then(expr.clone())
            .map(|(name, value)| Stmt::Assign { name, value });

        let if_stmt = recursive(|_| {
            let elseif_branch = just(Token::Elseif)
                .ignore_then(expr.clone())
                .then(block.clone());

            let else_branch = just(Token::Else).ignore_then(block.clone());

            just(Token::If)
                .ignore_then(expr.clone())
                .then(block.clone())
                .then(elseif_branch.repeated().collect::<Vec<_>>())
                .then(else_branch.or_not())
                .map(|(((cond, then_b), elseifs), final_else)| {
                    let mut current_else = final_else;
                    for (e_cond, e_body) in elseifs.into_iter().rev() {
                        current_else = Some(vec![Stmt::If {
                            condition: e_cond,
                            then_block: e_body,
                            else_block: current_else,
                        }]);
                    }

                    Stmt::If {
                        condition: cond,
                        then_block: then_b,
                        else_block: current_else,
                    }
                })
        });

        let param_parser = type_parser.clone().then(ident.clone()).map(|(t, n)| (t, n));

        let fn_params = param_parser
            .separated_by(just(Token::Comma))
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LParen), just(Token::RParen));

        let fn_stmt = type_parser
            .clone()
            .then(ident.clone())
            .then(fn_params)
            .then(block.clone())
            .map(|(((return_type, name), params), body)| Stmt::Function {
                return_type,
                name,
                params,
                body,
            });

        let return_stmt = just(Token::Return)
            .ignore_then(expr.clone().or_not()) // Clone it so it doesnt get Thanos snapped
            .map(Stmt::Return);

        let for_stmt = just(Token::For)
            .ignore_then(type_parser.clone())
            .then(ident.clone())
            .then_ignore(just(Token::Assign))
            .then(expr.clone())
            .then_ignore(just(Token::Comma))
            .then(expr.clone())
            .then_ignore(just(Token::Comma))
            .then(expr.clone())
            .then(block.clone())
            .map(
                |(((((var_type, name), start), end), step), body)| Stmt::For {
                    var_type,
                    name,
                    start,
                    end,
                    step,
                    body,
                },
            );

        fn_stmt
            .or(typed_let_stmt)
            .or(let_stmt)
            .or(assign_stmt)
            .or(if_stmt)
            .or(return_stmt)
            .or(for_stmt)
            .or(expr.map(Stmt::Expr))
    })
    .repeated()
    .collect()
}

pub fn parse_code(tokens: &[Token], source: &str) -> Result<Vec<Stmt>, Vec<ParseDiagnostic>> {
    use chumsky::Parser;

    match parser().parse(tokens).into_result() {
        Ok(ast) => Ok(ast),

        Err(errors) => {
            let mut diagnostics = Vec::new();

            for error in errors {
                let span = error.span().into_range();

                let message = match error.found() {
                    Some(token) => format!("unexpected {:?}", token),
                    None => "unexpected end of file".to_string(),
                };

                diagnostics.push(ParseDiagnostic {
                    rendered: render_diagnostic(
                        source,
                        "syntax error",
                        span.start,
                        span.end,
                        &message,
                        None,
                    ),
                    message,
                    start: error.span().start,
                    end: error.span().end,
                    help: None,
                });
            }

            Err(diagnostics)
        }
    }
}
