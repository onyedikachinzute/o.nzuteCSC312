use crate::ast::{Expr, Function, Program, Resolved, Stmt};

/// Binds every name to its declaration once, before either engine runs. Scoping is
/// lexical (spec section 4.2), so the answer is a property of the text and cannot change
/// between runs: the interpreter gets a depth to index with, the VM a stack slot.
///
/// Globals are deliberately left unresolved: one may be used before its `var` has run,
/// which is what makes mutual recursion legal, so a name that matches no local is a
/// global by construction. Spec section 7.
pub fn resolve(program: &mut Program) {
    for f in &mut program.functions {
        resolve_function(f);
    }
    let mut r = Resolver::default();
    r.statements(&mut program.stmts);
}

struct Local {
    name: String,
    scope: usize,
    /// False between `declare` and `define`, so a binding is invisible while its own
    /// initialiser is resolved and `var a = a + 1;` reads the enclosing `a` (section 4.2).
    initialised: bool,
}

/// Chapter 11 of *Crafting Interpreters* keys its scopes by name, `Vec<HashMap<String,
/// bool>>`. That cannot hold Kobo: section 4.2 licenses `{ var a = 1; var a = a + 1; }`, where
/// one scope has two live bindings of `a` and the second's initialiser reads the first.
/// A flat list holds both, carries the flag per binding, and gives the slot as its index.
#[derive(Default)]
struct Resolver {
    locals: Vec<Local>,
    depth: usize,
}

impl Resolver {
    /// Returns how many locals the scope held: the number of `Pop`s the bytecode
    /// compiler owes for leaving it.
    fn end_scope(&mut self) -> usize {
        let mut count = 0;
        while let Some(local) = self.locals.last() {
            if local.scope == self.depth {
                self.locals.pop();
                count += 1;
            } else {
                break;
            }
        }
        self.depth -= 1;
        count
    }

    fn declare(&mut self, name: &str) {
        if self.depth == 0 {
            return;
        }
        self.locals.push(Local {
            name: name.to_string(),
            scope: self.depth,
            initialised: false,
        });
    }

    /// `None` at the top level of the script, where a declaration is a global.
    fn define(&mut self, _name: &str) -> Option<usize> {
        if self.depth == 0 {
            return None;
        }
        let slot = self.locals.len() - 1;
        self.locals[slot].initialised = true;
        Some(slot)
    }

    /// Innermost first, so the nearest declaration wins and an outer one stays shadowed.
    fn lookup(&self, name: &str) -> Resolved {
        for (slot, local) in self.locals.iter().enumerate().rev() {
            if local.initialised && local.name == name {
                return Resolved::Local {
                    depth: self.depth - local.scope,
                    slot,
                };
            }
        }
        Resolved::Global
    }

    fn statements(&mut self, stmts: &mut [Stmt]) {
        for stmt in stmts {
            self.statement(stmt);
        }
    }

    fn statement(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Expression(e) | Stmt::Print(e) => self.expression(e),

            Stmt::Var {
                name,
                initialiser,
                slot,
            } => {
                // Declare, resolve, define, in that order, which is the whole of section 4.2.
                self.declare(&name.lexeme);
                if let Some(e) = initialiser {
                    self.expression(e);
                }
                *slot = self.define(&name.lexeme);
            }

            Stmt::Block { body, locals } => {
                self.depth += 1;
                self.statements(body);
                *locals = self.end_scope();
            }

            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.expression(condition);
                self.statement(then_branch);
                if let Some(alt) = else_branch {
                    self.statement(alt);
                }
            }

            Stmt::While { condition, body } => {
                self.expression(condition);
                self.statement(body);
            }

            // The body was resolved in its own frame above; only the name binds here.
            Stmt::Function { name, slot, .. } => {
                self.declare(name);
                *slot = self.define(name);
            }

            Stmt::Return(value) => {
                if let Some(e) = value {
                    self.expression(e);
                }
            }
        }
    }

    fn expression(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Literal { .. } => {}
            Expr::Grouping(inner) => self.expression(inner),

            Expr::Variable { name, at } => *at = self.lookup(&name.lexeme),

            Expr::Assign { name, value, at } => {
                self.expression(value);
                *at = self.lookup(&name.lexeme);
            }

            Expr::Call { callee, args, .. } => {
                self.expression(callee);
                for arg in args {
                    self.expression(arg);
                }
            }

            Expr::Unary { right, .. } => self.expression(right),

            Expr::Binary { left, right, .. } | Expr::Logical { left, right, .. } => {
                self.expression(left);
                self.expression(right);
            }
        }
    }
}
