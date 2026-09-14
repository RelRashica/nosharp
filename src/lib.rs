// This file is part of the No# (Nosharp) programming language and is licensed under MIT License; see LICENSE.txt for details

use logos::Logos;
use serde::Serialize;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

mod commands;
mod eval;
mod syntax;

use commands::{Command, lower_program};
use syntax::{Token, parse_code, render_diagnostic};

#[derive(Serialize)]
pub struct Diagnostic {
    pub message: String,
    pub start: usize,
    pub end: usize,
    pub help: Option<String>,
    pub rendered: Option<String>,
}

#[derive(Serialize)]
pub struct ParseResult {
    pub success: bool,
    pub commands: Option<Vec<Command>>,
    pub errors: Vec<Diagnostic>,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nosharp_parse(
    source_ptr: *const c_char,
    globals_c_char: *const c_char,
) -> *const c_char {
    if source_ptr.is_null() {
        return std::ptr::null();
    }

    let c_str = unsafe { CStr::from_ptr(source_ptr) };
    let code = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => {
            let err_res = ParseResult {
                success: false,
                commands: None,
                errors: vec![Diagnostic {
                    message: "Invalid UTF-8 string".into(),
                    start: 0,
                    end: 0,
                    help: None,
                    rendered: None,
                }],
            };

            return result_to_c_string(&err_res);
        }
    };

    let globals_str = if globals_c_char.is_null() {
        "{}"
    } else {
        match unsafe { CStr::from_ptr(globals_c_char) }.to_str() {
            Ok(s) => s,
            Err(_) => {
                let err_res = ParseResult {
                    success: false,
                    commands: None,
                    errors: vec![Diagnostic {
                        message: "Invalid UTF-8 globals JSON".into(),
                        start: 0,
                        end: 0,
                        help: None,
                        rendered: None,
                    }],
                };

                return result_to_c_string(&err_res);
            }
        }
    };

    let globals: HashMap<String, serde_json::Value> = match serde_json::from_str(globals_str) {
        Ok(globals) => globals,
        Err(err) => {
            let err_res = ParseResult {
                success: false,
                commands: None,
                errors: vec![Diagnostic {
                    message: format!("Invalid globals JSON: {err}"),
                    start: 0,
                    end: 0,
                    help: Some("Expected an object of global function metadata.".to_string()),
                    rendered: None,
                }],
            };

            return result_to_c_string(&err_res);
        }
    };

    let mut lex_errors = Vec::new();
    let tokens: Vec<Token> = Token::lexer(code)
        .spanned()
        .filter_map(|(res, span)| match res {
            Ok(token) => Some(token),
            Err(_) => {
                let help = "Remove or replace this character.";
                lex_errors.push(Diagnostic {
                    message: "unknown token".to_string(),
                    start: span.start,
                    end: span.end,
                    help: Some(help.to_string()),
                    rendered: render_diagnostic(
                        code,
                        "lex error",
                        span.start,
                        span.end,
                        "this character is not valid No# syntax",
                        Some(help),
                    ),
                });
                None
            }
        })
        .collect();

    if !lex_errors.is_empty() {
        let result = ParseResult {
            success: false,
            commands: None,
            errors: lex_errors,
        };

        return result_to_c_string(&result);
    }

    let ast = match parse_code(&tokens, code) {
        Ok(ast) => ast,
        Err(errors) => {
            let diag_errors = errors
                .into_iter()
                .map(|e| Diagnostic {
                    message: e.message,
                    start: e.start,
                    end: e.end,
                    help: e.help,
                    rendered: e.rendered,
                })
                .collect();

            let result = ParseResult {
                success: false,
                commands: None,
                errors: diag_errors,
            };

            return result_to_c_string(&result);
        }
    };

    let commands = lower_program(&ast);
    let check_errors = eval::check_program(&ast, globals)
        .into_iter()
        .map(|error| Diagnostic {
            message: error.message,
            start: error.start,
            end: error.end,
            help: error.help,
            rendered: error.rendered,
        })
        .collect::<Vec<_>>();

    if !check_errors.is_empty() {
        let result = ParseResult {
            success: false,
            commands: Some(commands),
            errors: check_errors,
        };

        return result_to_c_string(&result);
    }

    let result = ParseResult {
        success: true,
        commands: Some(commands),
        errors: vec![],
    };

    result_to_c_string(&result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nosharp_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

fn result_to_c_string(result: &ParseResult) -> *const c_char {
    let json = match serde_json::to_string(result) {
        Ok(json) => json,
        Err(err) => format!(
            "{{\"success\":false,\"commands\":null,\"errors\":[{{\"message\":\"failed to serialize result: {}\",\"start\":0,\"end\":0,\"help\":null,\"rendered\":null}}]}}",
            err
        ),
    };

    CString::new(json).unwrap().into_raw()
}
