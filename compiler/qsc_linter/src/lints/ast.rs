// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use super::lint;
use crate::linter::ast::declare_ast_lints;
use qsc_ast::ast::{BinOp, Block, Expr, ExprKind, Item, ItemKind, Lit, Stmt, StmtKind};
use qsc_data_structures::span::Span;

// Read Me:
//  To add a new lint add a new tuple to this structure. The tuple has four elements:
//  `(lint_name, default_lint_level, message, help)`
//
//  where:
//   lint_name: Name of the lint.
//   default_lint_level: Default level for the lint, can be overridden by the user in qsharp.json.
//   message: Message shown in the editor when hovering over the squiggle generated by the lint.
//   help: A user friendly message explaining how to fix the lint.
//
//  Then, add an `impl lint_name` block adding the lint logic.
//
//  After adding a lint you need to go language_service/code_action.rs and add a Quickfix
//  for the newly added lint (or opt out of the Quickfix) in the match statement in that file.
//
//  For more details on how to add a lint, please refer to the crate-level documentation
//  in qsc_linter/lib.rs.
declare_ast_lints! {
    (DivisionByZero, LintLevel::Error, "attempt to divide by zero", "division by zero will fail at runtime"),
    (NeedlessParens, LintLevel::Allow, "unnecessary parentheses", "remove the extra parentheses for clarity"),
    (RedundantSemicolons, LintLevel::Warn, "redundant semicolons", "remove the redundant semicolons"),
    (DeprecatedNewtype, LintLevel::Allow, "deprecated `newtype` declarations", "`newtype` declarations are deprecated, use `struct` instead"),
}

impl AstLintPass for DivisionByZero {
    fn check_expr(&self, expr: &Expr, buffer: &mut Vec<Lint>) {
        if let ExprKind::BinOp(BinOp::Div, _, ref rhs) = *expr.kind {
            if let ExprKind::Lit(ref lit) = *rhs.kind {
                if let Lit::Int(0) = **lit {
                    buffer.push(lint!(self, expr.span));
                }
            }
        }
    }
}

impl NeedlessParens {
    /// The idea is that if we find a expr of the form:
    /// a + (expr)
    /// and `expr` has higher precedence than `+`, then the
    /// parentheses are needless. Parentheses around a literal
    /// are also needless.
    fn push(&self, parent: &Expr, child: &Expr, buffer: &mut Vec<Lint>) {
        if let ExprKind::Paren(expr) = &*child.kind {
            if precedence(parent) < precedence(expr) {
                buffer.push(lint!(
                    self,
                    child.span,
                    Self::get_code_action_edits(child.span)
                ));
            }
        }
    }

    /// Returns the code action edits that strip out the first and last characters for the given span.
    fn get_code_action_edits(span: Span) -> Vec<(String, Span)> {
        vec![
            (
                String::new(), // Remove the lower `(`
                Span {
                    lo: span.lo,
                    hi: span.lo + 1,
                },
            ),
            (
                String::new(), // Remove the upper `)`
                Span {
                    lo: span.hi - 1,
                    hi: span.hi,
                },
            ),
        ]
    }
}

impl AstLintPass for NeedlessParens {
    fn check_expr(&self, expr: &Expr, buffer: &mut Vec<Lint>) {
        match &*expr.kind {
            ExprKind::BinOp(_, left, right) => {
                self.push(expr, left, buffer);
                self.push(expr, right, buffer);
            }
            ExprKind::Assign(_, right) | ExprKind::AssignOp(_, _, right) => {
                self.push(expr, right, buffer);
            }
            _ => (),
        }
    }

    /// Checks the assignment statements.
    fn check_stmt(&self, stmt: &Stmt, buffer: &mut Vec<Lint>) {
        if let StmtKind::Local(_, _, right) = &*stmt.kind {
            if let ExprKind::Paren(_) = &*right.kind {
                buffer.push(lint!(
                    self,
                    right.span,
                    Self::get_code_action_edits(right.span)
                ));
            }
        }
    }
}

impl RedundantSemicolons {
    /// Helper function that pushes a lint to the buffer if we have
    /// found two or more semicolons.
    fn maybe_push(&self, seq: &mut Option<Span>, buffer: &mut Vec<Lint>) {
        if let Some(span) = seq.take() {
            buffer.push(lint!(self, span, vec![(String::new(), span)]));
        }
    }
}

impl AstLintPass for RedundantSemicolons {
    /// Checks if there are redundant semicolons. The idea is that a redundant
    /// semicolon is parsed as an Empty statement. If we have multiple empty
    /// statements in a row, we group them as single lint, that spans from
    /// the first redundant semicolon to the last redundant semicolon.
    fn check_block(&self, block: &Block, buffer: &mut Vec<Lint>) {
        // a finite state machine that keeps track of the span of the redundant semicolons
        // None: no redundant semicolons
        // Some(_): one or more redundant semicolons
        let mut seq: Option<Span> = None;

        for stmt in block.stmts.iter() {
            match (&*stmt.kind, &mut seq) {
                (StmtKind::Empty, None) => seq = Some(stmt.span),
                (StmtKind::Empty, Some(span)) => span.hi = stmt.span.hi,
                (_, seq) => self.maybe_push(seq, buffer),
            }
        }

        self.maybe_push(&mut seq, buffer);
    }
}

fn precedence(expr: &Expr) -> u8 {
    match &*expr.kind {
        ExprKind::Lit(_) => 15,
        ExprKind::Paren(_) => 14,
        ExprKind::UnOp(_, _) => 13,
        ExprKind::BinOp(op, _, _) => match op {
            BinOp::Exp => 12,
            BinOp::Div | BinOp::Mod | BinOp::Mul => 10,
            BinOp::Add | BinOp::Sub => 9,
            BinOp::Shl | BinOp::Shr => 8,
            BinOp::AndB => 7,
            BinOp::XorB => 6,
            BinOp::OrB => 5,
            BinOp::Gt | BinOp::Gte | BinOp::Lt | BinOp::Lte | BinOp::Eq | BinOp::Neq => 4,
            BinOp::AndL => 3,
            BinOp::OrL => 2,
        },
        ExprKind::Assign(_, _) | ExprKind::AssignOp(_, _, _) => 1,
        _ => 0,
    }
}

/// Creates a lint for deprecated user-defined types declarations using `newtype`.
impl AstLintPass for DeprecatedNewtype {
    fn check_item(&self, item: &Item, buffer: &mut Vec<Lint>) {
        if let ItemKind::Ty(_, _) = item.kind.as_ref() {
            buffer.push(lint!(self, item.span));
        }
    }
}
