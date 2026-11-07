# CA2 Reflection — Parser

### Question 1: Dangling Else Resolution
In `src/parser.rs:170-174`, the dangling `else` is resolved inside `if_statement`:

```rust
let else_branch = if self.matches(&[TokenType::Else]) {
    Some(Box::new(self.statement()?))
} else {
    None
};
```

The recursion resolves ambiguity because self.statement() at src/parser.rs:169 is invoked recursively to parse then_branch. When an inner if statement executes, its own nested if_statement function eagerly checks for, matches, and consumes the else token at src/parser.rs:170 before returning control back up to the outer if_statement. The grammar specification itself is ambiguous, but the control flow of recursive descent forces the innermost if currently on the call stack to bind the else.

For if (false) if (true) print "a"; else print "b";, the parser prints:(if false (if true (print a) (print b)))

If the else bound to the outer if instead, it would print:(if false (if true (print a)) (print b))

Broken commit (aa0e160): let else_branch = None;

Fixed commit (f5edddd): let else_branch = if self.matches(&[TokenType::Else]) { Some(Box::new(self.statement()?)) } else { None };

### Question 2: Assignment Left-Hand Side Validation

In `src/parser.rs:230-238`, the check validating the target of an assignment occurs inside assignment:

```rust
if let Expr::Variable { name, at } = expr {
    return Ok(Expr::Assign {
        name,
        value: Box::new(value),
        at,
    });
}

self.error_at(&equals, "Cannot assign to this expression.");
```

This check cannot happen before parsing the left side because Kobo's grammar is not LL(1). Lookahead alone cannot distinguish an identifier assignment target from the beginning of a complex expression (such as a + b versus a = 1). Consequently, the left operand must first be fully parsed as a generic expression via self.or() at src/parser.rs:224, and only verified after encountering the = token at src/parser.rs:226.

For the invalid input 1 = 2;, the parser reports:[line 1] Error at '=': Cannot assign to this expression.

Broken commit (c4b2efb): self.or()

Fixed commit (f5edddd): if let Expr::Variable { name, at } = expr { return Ok(Expr::Assign { name, value: Box::new(value), at }); } self.error_at(&equals, "Cannot assign to this expression.");

### Question 3: Error Recovery via Synchronisation

In `src/parser.rs:445-469`, `synchronise` handles parse error recovery:

```rust
fn synchronise(&mut self) {
    self.depth = 0;
    if !self.at_end() {
        self.current += 1;
    }
    while !self.at_end() {
        if self.previous().kind == TokenType::Semicolon {
            return;
        }
        match self.peek().kind {
            TokenType::Fun
            | TokenType::Var
            | TokenType::If
            | TokenType::While
            | TokenType::Print
            | TokenType::Return => return,
            _ => {}
        }
        self.current += 1;
    }
}
```

synchronise discards tokens until it steps just past a SEMICOLON at src/parser.rs:453 or positions itself on a declaration/statement keyword (fun, var, if, while, print, return) at src/parser.rs:458-463. These tokens mark clean boundaries where statement parsing can safely resume without cascading failures.

In tests/phase-2/invalid/expect_semicolon_after_value.kobo:

print 1
print 2;

Standard error output with synchronise:[line 2] Error at 'print': Expect ';' after value.

If the call to synchronise() at src/parser.rs:18 is removed, the parser fails to discard tokens and make progress. Its output changes to:[line 2] Error at 'print': Expect ';' after value.[line 2] Error at ';': Expect expression.

Broken commit (d0f07b9): self.depth = 0; while !self.at_end() { self.current += 1; if self.previous().kind == TokenType::Semicolon { return; } }

Fixed commit (f5edddd): match self.peek().kind { TokenType::Fun | TokenType::Var | TokenType::If | TokenType::While | TokenType::Print | TokenType::Return => return, _ => {} }