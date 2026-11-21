use crate::ast::{Expr, Program, Resolved, Stmt};
use crate::environment::Environment;
use crate::token::{Token, TokenType};
use crate::value::Value;

pub struct RuntimeError {
    pub line: usize,
    pub message: String,
}

/// Why a statement stopped early. An error unwinds to `main`; a `return` unwinds only as
/// far as the call that started the function, which is why both travel the same way. Rust
/// gives `?` for free on a `Result`, and a `return` is exactly a non-local exit.
enum Flow {
    Error(RuntimeError),
    Return(Value),
}

type Exec<T> = Result<T, Flow>;

/// Kobo's call stack is the Rust call stack: `call` calls `execute` calls `evaluate`
/// calls `call`. Running off the end of it aborts the process with no diagnostic, so the
/// depth is capped instead. Measured on this toolchain, the real limit is near 1,500
/// frames; 500 leaves room for a deeply nested expression inside every one of them.
pub const MAX_FRAMES: usize = 500;

pub fn interpret(program: &Program) -> Result<(), RuntimeError> {
    let mut env = Environment::new();
    for stmt in &program.stmts {
        match execute(stmt, &mut env, program) {
            Ok(()) => {}
            // `return` outside any function ends the program (spec section 3.1).
            Err(Flow::Return(_)) => return Ok(()),
            Err(Flow::Error(e)) => return Err(e),
        }
    }
    Ok(())
}

fn execute(stmt: &Stmt, env: &mut Environment, program: &Program) -> Exec<()> {
    match stmt {
        Stmt::Expression(expr) => {
            evaluate(expr, env, program)?;
            Ok(())
        }
        Stmt::Print(expr) => {
            let val = evaluate(expr, env, program)?;
            println!("{}", val);
            Ok(())
        }
        Stmt::Var {
            name, initialiser, ..
        } => {
            let val = match initialiser {
                Some(expr) => evaluate(expr, env, program)?,
                None => Value::Nil,
            };
            env.declare(&name.lexeme, val);
            Ok(())
        }
        Stmt::Block { body, .. } => {
            env.push_scope();
            for s in body {
                if let Err(flow) = execute(s, env, program) {
                    env.pop_scope();
                    return Err(flow);
                }
            }
            env.pop_scope();
            Ok(())
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
        } => {
            if evaluate(condition, env, program)?.is_truthy() {
                execute(then_branch, env, program)?;
            } else if let Some(alt) = else_branch {
                execute(alt, env, program)?;
            }
            Ok(())
        }
        Stmt::While { condition, body } => {
            while evaluate(condition, env, program)?.is_truthy() {
                execute(body, env, program)?;
            }
            Ok(())
        }
        Stmt::Function { index, name, .. } => {
            env.declare(
                name,
                Value::Function {
                    index: *index,
                    name: name.clone(),
                },
            );
            Ok(())
        }
        Stmt::Return(expr) => {
            let val = match expr {
                Some(e) => evaluate(e, env, program)?,
                None => Value::Nil,
            };
            Err(Flow::Return(val))
        }
    }
}

fn evaluate(expr: &Expr, env: &mut Environment, program: &Program) -> Exec<Value> {
    match expr {
        Expr::Literal { value, .. } => Ok(value.clone()),
        Expr::Grouping(inner) => evaluate(inner, env, program),

        Expr::Variable { name, at } => match at {
            Resolved::Global => env
                .get_global(&name.lexeme)
                .cloned()
                .ok_or_else(|| undefined(name)),
            Resolved::Local { depth, .. } => env
                .get_at(*depth, &name.lexeme)
                .cloned()
                .ok_or_else(|| undefined(name)),
        },

        Expr::Assign { name, value, at } => {
            let val = evaluate(value, env, program)?;
            let assigned = match at {
                Resolved::Global => env.assign_global(&name.lexeme, val.clone()),
                Resolved::Local { depth, .. } => env.assign_at(*depth, &name.lexeme, val.clone()),
            };
            if assigned {
                Ok(val)
            } else {
                Err(undefined(name))
            }
        }

        Expr::Logical { left, op, right } => {
            let l = evaluate(left, env, program)?;
            match op.kind {
                TokenType::Or => {
                    if l.is_truthy() {
                        Ok(l)
                    } else {
                        evaluate(right, env, program)
                    }
                }
                TokenType::And => {
                    if !l.is_truthy() {
                        Ok(l)
                    } else {
                        evaluate(right, env, program)
                    }
                }
                // Added for extension (`??`):
                TokenType::QuestionQuestion => {
                    if !matches!(l, Value::Nil) {
                        Ok(l)
                    } else {
                        evaluate(right, env, program)
                    }
                }
                _ => unreachable!(),
            }
        }

        Expr::Unary { op, right } => {
            let r = evaluate(right, env, program)?;
            match op.kind {
                TokenType::Bang => Ok(Value::Bool(!r.is_truthy())),
                TokenType::Minus => match r {
                    Value::Number(n) => Ok(Value::Number(-n)),
                    _ => Err(err(op.line, "Operand must be a number.")),
                },
                _ => unreachable!(),
            }
        }

        Expr::Binary { left, op, right } => {
            let l = evaluate(left, env, program)?;
            let r = evaluate(right, env, program)?;
            match op.kind {
                TokenType::Plus => match (l, r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
                    (Value::Str(a), Value::Str(b)) => Ok(Value::Str(a + &b)),
                    _ => Err(err(op.line, "Operands must be two numbers or two strings.")),
                },
                TokenType::Minus => arith(l, r, op.line, |a, b| a - b),
                TokenType::Star => arith(l, r, op.line, |a, b| a * b),
                TokenType::Slash => {
                    let (a, b) = numbers(l, r, op.line)?;
                    if b == 0.0 {
                        Err(err(op.line, "Division by zero."))
                    } else {
                        Ok(Value::Number(a / b))
                    }
                }
                TokenType::Greater => compare(l, r, op.line, |a, b| a > b),
                TokenType::GreaterEqual => compare(l, r, op.line, |a, b| a >= b),
                TokenType::Less => compare(l, r, op.line, |a, b| a < b),
                TokenType::LessEqual => compare(l, r, op.line, |a, b| a <= b),
                TokenType::EqualEqual => Ok(Value::Bool(l == r)),
                TokenType::BangEqual => Ok(Value::Bool(l != r)),
                _ => unreachable!(),
            }
        }

        Expr::Call {
            callee,
            paren,
            args,
        } => {
            let callee_val = evaluate(callee, env, program)?;
            let mut arg_vals = Vec::with_capacity(args.len());
            for arg in args {
                arg_vals.push(evaluate(arg, env, program)?);
            }
            match callee_val {
                Value::Function { index, .. } => call(index, arg_vals, paren.line, env, program),
                _ => Err(err(paren.line, "This value is not a function.")),
            }
        }
    }
}

/// Run a function body in a fresh frame. Everything about Kobo's scope rules is in these
/// twenty lines: the parameters are declared in the frame's own scope, the body can reach
/// past it only to the globals, and the frame is popped on every path out: normal
/// completion, `return`, or a runtime error.
fn call(
    index: usize,
    args: Vec<Value>,
    line: usize,
    env: &mut Environment,
    program: &Program,
) -> Exec<Value> {
    let func = &program.functions[index];
    if args.len() != func.params.len() {
        return Err(err(
            line,
            &format!(
                "Expected {} arguments but got {}.",
                func.params.len(),
                args.len()
            ),
        ));
    }
    if env.frame_depth() >= MAX_FRAMES {
        return Err(err(line, "Stack overflow."));
    }

    env.push_frame();
    for (param, arg) in func.params.iter().zip(args) {
        env.declare(param, arg);
    }

    let mut result = Value::Nil;
    for stmt in &func.body {
        match execute(stmt, env, program) {
            Ok(()) => {}
            Err(Flow::Return(v)) => {
                result = v;
                break;
            }
            Err(Flow::Error(e)) => {
                env.pop_frame();
                return Err(Flow::Error(e));
            }
        }
    }

    env.pop_frame();
    Ok(result)
}

fn numbers(l: Value, r: Value, line: usize) -> Exec<(f64, f64)> {
    match (l, r) {
        (Value::Number(a), Value::Number(b)) => Ok((a, b)),
        _ => Err(err(line, "Operands must be numbers.")),
    }
}

fn arith(l: Value, r: Value, line: usize, f: fn(f64, f64) -> f64) -> Exec<Value> {
    let (a, b) = numbers(l, r, line)?;
    Ok(Value::Number(f(a, b)))
}

fn compare(l: Value, r: Value, line: usize, f: fn(f64, f64) -> bool) -> Exec<Value> {
    let (a, b) = numbers(l, r, line)?;
    Ok(Value::Bool(f(a, b)))
}

fn undefined(name: &Token) -> Flow {
    err(name.line, &format!("Variable '{}' is not defined.", name.lexeme))
}

fn err(line: usize, message: &str) -> Flow {
    Flow::Error(RuntimeError {
        line,
        message: message.to_string(),
    })
}
