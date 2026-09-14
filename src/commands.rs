// This file is part of the No# (Nosharp) programming language and is licensed under MIT License; see LICENSE.txt for details

use crate::syntax::{Expr, Op, Stmt};
use serde::Serialize;

// A simple value that C# can understand
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Value {
    Int(i32),
    Float(f32),
    String(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum CsExpr {
    Literal {
        value: Value,
    },
    Variable {
        name: String,
    },
    BinaryOp {
        op: String,
        left: Box<CsExpr>,
        right: Box<CsExpr>,
    },
    UnaryOp {
        op: String,
        operand: Box<CsExpr>,
    },
    Call {
        name: String,
        args: Vec<CsExpr>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum Command {
    SetVar {
        name: String,
        var_type: Option<String>,
        value: CsExpr,
    },
    AssignVar {
        name: String,
        value: CsExpr,
    },
    CallFunction {
        name: String,
        args: Vec<CsExpr>,
    },
    DefineFunction {
        name: String,
        return_type: String,
        params: Vec<(String, String)>, // (type, name)
        body: Vec<Command>,
    },
    If {
        condition: CsExpr,
        then_block: Vec<Command>,
        else_block: Option<Vec<Command>>,
    },
    For {
        var_type: String,
        name: String,
        start: CsExpr,
        end: CsExpr,
        step: CsExpr,
        body: Vec<Command>,
    },
    Return {
        value: Option<CsExpr>,
    },
    ExprStatement {
        value: CsExpr,
    },
}

pub fn lower_expr(expr: &Expr) -> CsExpr {
    match expr {
        Expr::Int(n) => CsExpr::Literal {
            value: Value::Int(*n),
        },
        Expr::Float(f) => CsExpr::Literal {
            value: Value::Float(*f),
        },
        Expr::String(s) => CsExpr::Literal {
            value: Value::String(s.clone()),
        },
        Expr::Bool(b) => CsExpr::Literal {
            value: Value::Bool(*b),
        },
        Expr::Var(name) => CsExpr::Variable { name: name.clone() },

        Expr::Unary { op, operand } => CsExpr::UnaryOp {
            op: op_to_string(op),
            operand: Box::new(lower_expr(operand)),
        },

        Expr::Binary { op, left, right } => CsExpr::BinaryOp {
            op: op_to_string(op),
            left: Box::new(lower_expr(left)),
            right: Box::new(lower_expr(right)),
        },

        Expr::Call { callee, args } => {
            let name = match callee.as_ref() {
                Expr::Var(n) => n.clone(),
                _ => "unknown".into(),
            };
            CsExpr::Call {
                name,
                args: args.iter().map(lower_expr).collect(),
            }
        }
    }
}

pub fn lower_stmt(stmt: &Stmt) -> Command {
    match stmt {
        Stmt::Let {
            var_type,
            name,
            value,
        } => Command::SetVar {
            name: name.clone(),
            var_type: var_type.clone(),
            value: lower_expr(value),
        },

        Stmt::Assign { name, value } => Command::AssignVar {
            name: name.clone(),
            value: lower_expr(value),
        },

        Stmt::Expr(expr) => match expr {
            Expr::Call { callee, args } => {
                let name = match callee.as_ref() {
                    Expr::Var(n) => n.clone(),
                    _ => "unknown".into(),
                };
                Command::CallFunction {
                    name,
                    args: args.iter().map(lower_expr).collect(),
                }
            }
            _ => Command::ExprStatement {
                value: lower_expr(expr),
            },
        },

        Stmt::Function {
            return_type,
            name,
            params,
            body,
        } => Command::DefineFunction {
            name: name.clone(),
            return_type: return_type.clone(),
            params: params.clone(),
            body: body.iter().map(lower_stmt).collect(),
        },

        Stmt::If {
            condition,
            then_block,
            else_block,
        } => Command::If {
            condition: lower_expr(condition),
            then_block: then_block.iter().map(lower_stmt).collect(),
            else_block: else_block
                .as_ref()
                .map(|b| b.iter().map(lower_stmt).collect()),
        },

        Stmt::For {
            var_type,
            name,
            start,
            end,
            step,
            body,
        } => Command::For {
            var_type: var_type.clone(),
            name: name.clone(),
            start: lower_expr(start),
            end: lower_expr(end),
            step: lower_expr(step),
            body: body.iter().map(lower_stmt).collect(),
        },

        Stmt::Return(expr) => Command::Return {
            value: expr.as_ref().map(lower_expr),
        },
    }
}

fn op_to_string(op: &Op) -> String {
    match op {
        Op::Add => "+",
        Op::Sub => "-",
        Op::Mul => "*",
        Op::Div => "/",
        Op::Equal => "==",
        Op::LessThan => "<",
        Op::MoreThan => ">",
        Op::LessOrEqual => "<=",
        Op::MoreOrEqual => ">=",
        Op::And => "and",
        Op::Or => "or",
        Op::Not => "not",
        Op::NotEqual => "!=",
        Op::Concat => "..",
    }
    .into()
}

pub fn lower_program(stmts: &[Stmt]) -> Vec<Command> {
    stmts.iter().map(lower_stmt).collect()
}
