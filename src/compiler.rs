use crate::ast::{Expr, Program, Resolved, Stmt};
use crate::chunk::{Bytecode, Chunk, Op};
use crate::token::TokenType;
use crate::value::Value;

pub fn compile(program: &Program) -> Bytecode {
    let functions = program
        .functions
        .iter()
        .map(|f| {
            let mut c = Compiler::new(Chunk::new(f.name.clone(), f.params.len()), program);
            c.in_function = true;
            c.statements(&f.body);
            c.finish(f.line)
        })
        .collect();

    let mut script = Compiler::new(Chunk::new("script".to_string(), 0), program);
    script.statements(&program.stmts);
    let line = script.chunk.last_line();
    Bytecode {
        script: script.finish(line),
        functions,
    }
}

struct Compiler<'a> {
    chunk: Chunk,
    program: &'a Program,
    in_function: bool,
}

impl<'a> Compiler<'a> {
    fn new(chunk: Chunk, program: &'a Program) -> Self {
        Compiler {
            chunk,
            program,
            in_function: false,
        }
    }

    fn finish(mut self, line: usize) -> Chunk {
        self.chunk.write(Op::Nil, line);
        self.chunk.write(Op::Return, line);
        self.chunk
    }

    fn statements(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.statement(stmt);
        }
    }

    // --- statements -------------------------------------------------------------------

    fn statement(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Expression(expr) => {
                let line = self.expression(expr);
                self.chunk.write(Op::Pop, line);
            }
            Stmt::Print(expr) => {
                let line = self.expression(expr);
                self.chunk.write(Op::Print, line);
            }
            Stmt::Var {
                name,
                initialiser,
                slot,
            } => {
                let line = match initialiser {
                    Some(expr) => self.expression(expr),
                    None => {
                        self.chunk.write(Op::Nil, name.line);
                        name.line
                    }
                };
                self.declare(&name.lexeme, *slot, line);
            }
            Stmt::Block { body, locals } => {
                self.statements(body);
                self.end_scope(*locals);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let line = self.expression(condition);
                let then_jump = self.chunk.write(Op::JumpIfFalse(0), line);
                self.chunk.write(Op::Pop, line);
                self.statement(then_branch);

                let else_jump = self.chunk.write(Op::Jump(0), line);
                self.patch(then_jump);
                self.chunk.write(Op::Pop, line);

                if let Some(alt) = else_branch {
                    self.statement(alt);
                }
                self.patch(else_jump);
            }
            Stmt::While { condition, body } => {
                let loop_start = self.chunk.code.len();
                let line = self.expression(condition);
                let exit_jump = self.chunk.write(Op::JumpIfFalse(0), line);
                self.chunk.write(Op::Pop, line);

                self.statement(body);
                self.chunk.write(Op::Jump(loop_start), line);

                self.patch(exit_jump);
                self.chunk.write(Op::Pop, line);
            }
            Stmt::Function { index, name, slot } => {
                let line = self.program.functions[*index].line;
                let idx = self.chunk.constant(Value::Function {
                    index: *index,
                    name: name.clone(),
                });
                self.chunk.write(Op::Constant(idx), line);
                self.declare(name, *slot, line);
            }
            Stmt::Return(value) => {
                let line = match value {
                    Some(expr) => self.expression(expr),
                    None => {
                        let l = self.chunk.last_line();
                        self.chunk.write(Op::Nil, l);
                        l
                    }
                };
                self.chunk.write(Op::Return, line);
            }
        }
    }

    // --- expressions ------------------------------------------------------------------

    fn expression(&mut self, expr: &Expr) -> usize {
        match expr {
            Expr::Literal { value, line } => {
                match value {
                    Value::Nil => self.chunk.write(Op::Nil, *line),
                    Value::Bool(true) => self.chunk.write(Op::True, *line),
                    Value::Bool(false) => self.chunk.write(Op::False, *line),
                    _ => {
                        let idx = self.chunk.constant(value.clone());
                        self.chunk.write(Op::Constant(idx), *line)
                    }
                };
                *line
            }
            Expr::Grouping(inner) => self.expression(inner),

            Expr::Variable { name, at } => {
                match at {
                    Resolved::Global => {
                        let idx = self.name_constant(&name.lexeme);
                        self.chunk.write(Op::GetGlobal(idx), name.line);
                    }
                    Resolved::Local { slot, .. } => {
                        self.chunk.write(Op::GetLocal(*slot), name.line);
                    }
                }
                name.line
            }

            Expr::Assign { name, value, at } => {
                self.expression(value);
                match at {
                    Resolved::Global => {
                        let idx = self.name_constant(&name.lexeme);
                        self.chunk.write(Op::SetGlobal(idx), name.line);
                    }
                    Resolved::Local { slot, .. } => {
                        self.chunk.write(Op::SetLocal(*slot), name.line);
                    }
                }
                name.line
            }

            Expr::Logical { left, op, right } => {
                self.expression(left);
                match op.kind {
                    TokenType::And => {
                        let end_jump = self.chunk.write(Op::JumpIfFalse(0), op.line);
                        self.chunk.write(Op::Pop, op.line);
                        self.expression(right);
                        self.patch(end_jump);
                    }
                    TokenType::Or => {
                        let else_jump = self.chunk.write(Op::JumpIfFalse(0), op.line);
                        let end_jump = self.chunk.write(Op::Jump(0), op.line);
                        self.patch(else_jump);
                        self.chunk.write(Op::Pop, op.line);
                        self.expression(right);
                        self.patch(end_jump);
                    }
                    // Added for extension (`??`):
                    TokenType::QuestionQuestion => {
                        let end_jump = self.chunk.write(Op::JumpIfNotNil(0), op.line);
                        self.chunk.write(Op::Pop, op.line);
                        self.expression(right);
                        self.patch(end_jump);
                    }
                    _ => unreachable!(),
                }
                op.line
            }

            Expr::Unary { op, right } => {
                self.expression(right);
                match op.kind {
                    TokenType::Bang => self.chunk.write(Op::Not, op.line),
                    TokenType::Minus => self.chunk.write(Op::Negate, op.line),
                    _ => unreachable!(),
                };
                op.line
            }

            Expr::Binary { left, op, right } => {
                self.expression(left);
                self.expression(right);
                let instr = match op.kind {
                    TokenType::Plus => Op::Add,
                    TokenType::Minus => Op::Subtract,
                    TokenType::Star => Op::Multiply,
                    TokenType::Slash => Op::Divide,
                    TokenType::EqualEqual => Op::Equal,
                    TokenType::BangEqual => Op::NotEqual,
                    TokenType::Greater => Op::Greater,
                    TokenType::GreaterEqual => Op::GreaterEqual,
                    TokenType::Less => Op::Less,
                    TokenType::LessEqual => Op::LessEqual,
                    _ => unreachable!(),
                };
                self.chunk.write(instr, op.line);
                op.line
            }

            Expr::Call {
                callee,
                paren,
                args,
            } => {
                self.expression(callee);
                for arg in args {
                    self.expression(arg);
                }
                self.chunk.write(Op::Call(args.len()), paren.line);
                paren.line
            }
        }
    }

    // --- names ------------------------------------------------------------------------

    fn declare(&mut self, name: &str, slot: Option<usize>, line: usize) {
        if slot.is_none() && !self.in_function {
            let idx = self.name_constant(name);
            self.chunk.write(Op::DefineGlobal(idx), line);
        }
    }

    fn name_constant(&mut self, name: &str) -> usize {
        self.chunk.constant(Value::Str(name.to_string()))
    }

    fn end_scope(&mut self, locals: usize) {
        let line = self.chunk.last_line();
        for _ in 0..locals {
            self.chunk.write(Op::Pop, line);
        }
    }

    fn patch(&mut self, at: usize) {
        let target = self.chunk.code.len();
        match &mut self.chunk.code[at] {
            Op::Jump(t) | Op::JumpIfFalse(t) | Op::JumpIfNotNil(t) => *t = target,
            _ => panic!("attempted to patch a non-jump instruction"),
        }
    }
}