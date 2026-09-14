// This file is part of the No# (Nosharp) programming language and is licensed under MIT License; see LICENSE.txt for details

use std::collections::HashMap;

use serde::Deserialize;

use crate::syntax::{Expr, Op, Stmt};

#[derive(Debug, Clone, PartialEq, Eq)]
enum TypeInfo {
    Int,
    Float,
    String,
    Bool,
    Void,
    Auto,
    Any,
    Custom(String),
    Unknown,
}

impl TypeInfo {
    fn from_name(name: &str) -> Self {
        match name {
            "int" => Self::Int,
            "float" => Self::Float,
            "string" => Self::String,
            "bool" => Self::Bool,
            "void" => Self::Void,
            "auto" => Self::Auto,
            "any" => Self::Any,
            other => Self::Custom(other.to_string()),
        }
    }

    fn name(&self) -> String {
        match self {
            Self::Int => "int".to_string(),
            Self::Float => "float".to_string(),
            Self::String => "string".to_string(),
            Self::Bool => "bool".to_string(),
            Self::Void => "void".to_string(),
            Self::Auto => "auto".to_string(),
            Self::Any => "any".to_string(),
            Self::Custom(name) => name.clone(),
            Self::Unknown => "unknown".to_string(),
        }
    }

    fn matches(&self, actual: &Self) -> bool {
        match (self, actual) {
            (Self::Auto, _)
            | (Self::Any, _)
            | (Self::Unknown, _)
            | (_, Self::Any)
            | (_, Self::Unknown) => true,
            _ => self == actual,
        }
    }

    fn is_number(&self) -> bool {
        match self {
            Self::Int | Self::Float => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
struct FunctionSig {
    return_type: TypeInfo,
    params: Vec<(TypeInfo, String)>,
}

#[derive(Debug, Clone)]
struct Scope {
    vars: HashMap<String, TypeInfo>,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub start: usize,
    pub end: usize,
    pub help: Option<String>,
    pub rendered: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Checker {
    scopes: Vec<Scope>,
    functions: HashMap<String, FunctionSig>,
    diagnostics: Vec<Diagnostic>,
    current_return_type: Option<TypeInfo>,
}

#[derive(Debug, Deserialize)]
struct GlobalFunction {
    #[serde(default)]
    parameters: Vec<String>,
    #[serde(default)]
    returns: Option<String>,
}

impl Checker {
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope {
                vars: HashMap::new(),
            }],
            functions: HashMap::new(),
            diagnostics: Vec::new(),
            current_return_type: None,
        }
    }

    pub fn register_globals(&mut self, globals: HashMap<String, serde_json::Value>) {
        for (name, value) in globals {
            match serde_json::from_value::<GlobalFunction>(value) {
                Ok(global) => {
                    let params = global
                        .parameters
                        .into_iter()
                        .enumerate()
                        .map(|(index, ty)| (TypeInfo::from_name(&ty), format!("arg{index}")))
                        .collect();
                    let return_type =
                        TypeInfo::from_name(global.returns.as_deref().unwrap_or("void"));

                    self.functions.insert(
                        name,
                        FunctionSig {
                            return_type,
                            params,
                        },
                    );
                }
                Err(err) => self.error(
                    format!("global `{name}` has invalid metadata: {err}"),
                    Some(
                        "Expected JSON like { \"parameters\": [\"int\"], \"returns\": \"void\" }"
                            .to_string(),
                    ),
                ),
            }
        }
    }

    pub fn check_program(mut self, statements: &[Stmt]) -> Vec<Diagnostic> {
        self.collect_function_signatures(statements);

        for stmt in statements {
            self.check_stmt(stmt);
        }

        self.diagnostics
    }

    fn collect_function_signatures(&mut self, statements: &[Stmt]) {
        for stmt in statements {
            if let Stmt::Function {
                return_type,
                name,
                params,
                ..
            } = stmt
            {
                if self.functions.contains_key(name) {
                    self.error(format!("function `{name}` is already defined"), None);
                }

                let params = params
                    .iter()
                    .map(|(ty, name)| (TypeInfo::from_name(ty), name.clone()))
                    .collect();

                self.functions.insert(
                    name.clone(),
                    FunctionSig {
                        return_type: TypeInfo::from_name(return_type),
                        params,
                    },
                );
            }
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                var_type,
                name,
                value,
            } => {
                let actual = self.check_expr(value);
                let declared = var_type
                    .as_deref()
                    .map(TypeInfo::from_name)
                    .unwrap_or(actual.clone());

                if !declared.matches(&actual) {
                    self.error(
                        format!(
                            "variable `{name}` is declared as `{}`, but got `{}`",
                            declared.name(),
                            actual.name()
                        ),
                        Some("Change the variable type or assign a matching value.".to_string()),
                    );
                }

                self.define_var(name.clone(), declared);
            }
            Stmt::Assign { name, value } => {
                let actual = self.check_expr(value);

                match self.lookup_var(name) {
                    Some(expected) if !expected.matches(&actual) => self.error(
                        format!(
                            "variable `{name}` is `{}`, but got `{}`",
                            expected.name(),
                            actual.name()
                        ),
                        Some("Assignments must keep the same type.".to_string()),
                    ),
                    Some(_) => {}
                    None => self.error(
                        format!("you can't change `{name}` because it does not exist yet"),
                        Some(format!(
                            "Create it first with `auto {name} = ...` or a typed declaration."
                        )),
                    ),
                }
            }
            Stmt::Function {
                return_type,
                params,
                body,
                ..
            } => {
                self.push_scope();
                let previous_return = self
                    .current_return_type
                    .replace(TypeInfo::from_name(return_type));

                for (ty, name) in params {
                    self.define_var(name.clone(), TypeInfo::from_name(ty));
                }

                for stmt in body {
                    self.check_stmt(stmt);
                }

                self.current_return_type = previous_return;
                self.pop_scope();
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                let condition_type = self.check_expr(condition);
                if condition_type != TypeInfo::Bool && condition_type != TypeInfo::Unknown {
                    self.error(
                        format!("an `if` needs `bool`, but got `{}`", condition_type.name()),
                        Some("Use a bool value or a comparison like `a == b`.".to_string()),
                    );
                }

                self.check_block(then_block);

                if let Some(else_block) = else_block {
                    self.check_block(else_block);
                }
            }
            Stmt::For {
                var_type,
                name,
                start,
                end,
                step,
                body,
            } => {
                for (label, expr) in [("start", start), ("end", end), ("step", step)] {
                    let ty = self.check_expr(expr);
                    if ty != TypeInfo::Int && ty != TypeInfo::Unknown {
                        self.error(
                            format!("for-loop {label} must be `int`, but got `{}`", ty.name()),
                            None,
                        );
                    }
                }

                self.push_scope();
                self.define_var(name.clone(), TypeInfo::from_name(var_type));
                self.check_block(body);
                self.pop_scope();
            }
            Stmt::Expr(expr) => {
                self.check_expr(expr);
            }
            Stmt::Return(expr) => {
                let actual = expr
                    .as_ref()
                    .map(|expr| self.check_expr(expr))
                    .unwrap_or(TypeInfo::Void);

                match &self.current_return_type {
                    Some(expected) if !expected.matches(&actual) => self.error(
                        format!(
                            "this function promised `{}`, but returned `{}`",
                            expected.name(),
                            actual.name()
                        ),
                        Some(
                            "Change the function return type or return a matching value."
                                .to_string(),
                        ),
                    ),
                    Some(_) => {}
                    None => self.error(
                        "`return` can only be used inside a function".to_string(),
                        None,
                    ),
                }
            }
        }
    }

    fn check_block(&mut self, statements: &[Stmt]) {
        self.push_scope();
        for stmt in statements {
            self.check_stmt(stmt);
        }
        self.pop_scope();
    }

    fn check_expr(&mut self, expr: &Expr) -> TypeInfo {
        match expr {
            Expr::Int(_) => TypeInfo::Int,
            Expr::Float(_) => TypeInfo::Float,
            Expr::String(_) => TypeInfo::String,
            Expr::Bool(_) => TypeInfo::Bool,
            Expr::Var(name) => self
                .lookup_var(name)
                .or_else(|| {
                    self.error(
                        format!("couldn't find a variable or function named `{name}`"),
                        Some(format!("Define `{name}` before using it.")),
                    );
                    None
                })
                .unwrap_or(TypeInfo::Unknown),
            Expr::Call { callee, args } => self.check_call(callee, args),
            Expr::Unary { op, operand } => {
                let operand_type = self.check_expr(operand);

                match (op, operand_type) {
                    (Op::Not, TypeInfo::Bool) => TypeInfo::Bool,
                    (Op::Not, TypeInfo::Unknown) => TypeInfo::Unknown,
                    (Op::Not, other) => {
                        self.error(
                            format!("`not` needs `bool`, but got `{}`", other.name()),
                            None,
                        );
                        TypeInfo::Unknown
                    }
                    (_, other) => {
                        self.error(
                            format!("unsupported unary operator for `{}`", other.name()),
                            None,
                        );
                        TypeInfo::Unknown
                    }
                }
            }
            Expr::Binary { op, left, right } => {
                let left_type = self.check_expr(left);
                let right_type = self.check_expr(right);
                self.check_binary(op, left_type, right_type)
            }
        }
    }

    fn check_call(&mut self, callee: &Expr, args: &[Expr]) -> TypeInfo {
        let name = match callee {
            Expr::Var(name) => name,
            _ => {
                self.check_expr(callee);
                self.error("only named functions can be called".to_string(), None);
                return TypeInfo::Unknown;
            }
        };

        let arg_types: Vec<TypeInfo> = args.iter().map(|arg| self.check_expr(arg)).collect();

        match self.functions.get(name).cloned() {
            Some(function) => {
                if arg_types.len() != function.params.len() {
                    self.error(
                        format!(
                            "`{name}` needs {} arguments, but got {}",
                            function.params.len(),
                            arg_types.len()
                        ),
                        Some(
                            "Pass the exact number of arguments the function declares.".to_string(),
                        ),
                    );
                }

                for (index, (actual, (expected, _))) in
                    arg_types.iter().zip(function.params.iter()).enumerate()
                {
                    if !expected.matches(actual) {
                        self.error(
                            format!(
                                "argument {} for `{name}` needs `{}`, but got `{}`",
                                index + 1,
                                expected.name(),
                                actual.name()
                            ),
                            None,
                        );
                    }
                }

                function.return_type
            }
            None => {
                self.error(
                    format!("couldn't find a function named `{name}`"),
                    Some("Declare the function or pass it in the globals JSON.".to_string()),
                );
                TypeInfo::Unknown
            }
        }
    }

    fn check_binary(&mut self, op: &Op, left: TypeInfo, right: TypeInfo) -> TypeInfo {
        if left == TypeInfo::Unknown || right == TypeInfo::Unknown {
            return TypeInfo::Unknown;
        }

        match op {
            Op::Add | Op::Sub | Op::Mul | Op::Div if left.is_number() && right.is_number() => {
                if left == TypeInfo::Float || right == TypeInfo::Float {
                    TypeInfo::Float
                } else {
                    TypeInfo::Int
                }
            }
            Op::Concat if left == TypeInfo::String && right == TypeInfo::String => TypeInfo::String,
            Op::Equal | Op::NotEqual if left.matches(&right) => TypeInfo::Bool,
            Op::LessThan | Op::MoreThan | Op::LessOrEqual | Op::MoreOrEqual
                if left.is_number() && right.is_number() =>
            {
                TypeInfo::Bool
            }
            Op::And | Op::Or if left == TypeInfo::Bool && right == TypeInfo::Bool => TypeInfo::Bool,
            _ => {
                self.error(
                    format!(
                        "you can't use `{:?}` with `{}` and `{}`",
                        op,
                        left.name(),
                        right.name()
                    ),
                    Some("Use compatible types for this operator.".to_string()),
                );
                TypeInfo::Unknown
            }
        }
    }

    fn define_var(&mut self, name: String, ty: TypeInfo) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.vars.insert(name, ty);
        }
    }

    fn lookup_var(&self, name: &str) -> Option<TypeInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.vars.get(name) {
                return Some(ty.clone());
            }
        }

        if self.functions.contains_key(name) {
            Some(TypeInfo::Custom("function".to_string()))
        } else {
            None
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(Scope {
            vars: HashMap::new(),
        });
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn error(&mut self, message: String, help: Option<String>) {
        self.diagnostics.push(Diagnostic {
            message,
            start: 0,
            end: 0,
            help,
            rendered: None,
        });
    }
}

pub fn check_program(
    statements: &[Stmt],
    globals: HashMap<String, serde_json::Value>,
) -> Vec<Diagnostic> {
    let mut checker = Checker::new();
    checker.register_globals(globals);
    checker.check_program(statements)
}
