# Reflection

## 1. Short-circuiting `and` and `or`

In `src/interpreter.rs:140-156`, `Expr::Logical` evaluates the left operand first and checks `l.is_truthy()` before touching `right`. For `TokenType::Or`, if `l.is_truthy()` is true it returns `Ok(l)` immediately without evaluating `right`; otherwise it evaluates and returns `right`. For `TokenType::And`, if `!l.is_truthy()` is true it returns `Ok(l)` immediately; otherwise it evaluates and returns `right`. Because Kobo defines only `false` and `nil` as falsey (`Value::is_truthy` in `src/value.rs:24-26`), `nil or "fallback"` skips `nil` and evaluates to `"fallback"`, whereas `0 and 1` treats `0` as truthy and evaluates to `1`. Returning `true` or `false` instead would pass tests where logical expressions are only consumed by `if` or `while` conditions (which only branch on truthiness), but would fail tests that print, assign, or pass the resulting operand value directly (such as `print nil or "fallback";`).

## 2. Static resolution and stack discipline

In `src/resolver.rs:64-72`, `lookup` iterates backwards over `self.locals` from the innermost scope outward, returning `Resolved::Local { depth, slot }` for the first initialised matching name or falling back to `Resolved::Global`. Resolving locals to a fixed stack slot at compile time makes the VM faster than the tree-walking interpreter because `Op::GetLocal(slot)` and `Op::SetLocal(slot)` (`src/vm.rs:79-85`) index directly into `self.stack[base + slot]` in $O(1)$ time, avoiding heap-allocated `HashMap` lookups and string comparisons at runtime. In `src/compiler.rs:269-274`, `end_scope` emits one `Op::Pop` per local declared inside a closing block. If `end_scope` emits one `Pop` too few, a dead local remains stranded on the VM stack. `phase-3` (the tree walker, which drops an entire scope map at once) does not catch this bug, whereas `phase-4` (`--vm`) catches it when subsequent local slot indices or function return boundaries shift out of alignment.

## 3. Extension: `??` (nil-coalescing)

I added `TokenType::QuestionQuestion` in `src/token.rs:39` and `src/token.rs:84`, scanning for `??` in `src/scanner.rs:103-109`, `Parser::nil_coalesce` below `or` in `src/parser.rs:226-240`, evaluation in `src/interpreter.rs:157-163`, `Op::JumpIfNotNil` in `src/chunk.rs:56` and `src/disassembler.rs:40`, bytecode emission in `src/compiler.rs:201-206`, and VM execution in `src/vm.rs:144-148`.

The bytecode path was harder than the tree walk because Kobo's existing conditional jump (`Op::JumpIfFalse` in `src/vm.rs:138-142`) branches on `!self.peek().is_truthy()`, which treats both `false` and `nil` as falsey. Reusing `JumpIfFalse` would have wrongly discarded `false` in `false ?? 1`. I therefore introduced a dedicated `Op::JumpIfNotNil` instruction that peeks at the top of the stack and jumps over the `Pop` and right-hand operand only when the left value is not `Value::Nil`.

In commit `91714c1`, `src/parser.rs:212` still routed assignment directly into `or()`, so `??` had no precedence rule and failed to parse:
```rust
let expr = self.or()?;
```
In commit `42e150d`, I updated `src/parser.rs:212` to call `nil_coalesce()` just below `or`:
```rust
let expr = self.nil_coalesce()?;
```