# Member 2: Standard Library & Effects (Week 1 Deliverables)

**Crate Ownership:** `crates/stdlib`
**Mission:** Our language runs on a JIT, but all standard library functions (like mapping over a list, or printing to the console) are actually pure Rust functions exposed to the language. Your job is to build the functional core.

## The Workflow & TDD Strategy

The Lead Architect will provide you with a `Value` enum (representing how data lives in memory: `Value::Int`, `Value::List`, `Value::Closure`) and a `BuiltinFunction` trait on Day 2. You will implement this trait for every standard library function using TDD.

### Your API Contract (Provided by Lead on Day 2)

```rust
// You will implement this for dozens of functions
pub trait BuiltinFunction {
    fn name(&self) -> SmolStr;
    fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError>;
}
```

## Week 1 Deliverables & Timeline

### Days 1-2: List Operations (The Functional Core)

- **Deliverable 1: List Built-ins.** Implement `List.map`, `List.filter`, `List.fold`, `List.len`, `List.head`, `List.tail`.
- **TDD Focus:** You don't need the language to test this! Manually construct `Value::List` arrays in Rust tests. Pass a mock closure to your `execute` function and assert the output is a correctly transformed `Value::List`.

### Days 3-4: Option & Result Handling

- **Deliverable 2: Option Operations.** Implement `Option.map`, `Option.unwrap`, `Option.isSome`.
- **Deliverable 3: Result Operations.** Implement `Result.map`, `Result.flatMap`.
- **TDD Focus:** Write exhaustive unit tests verifying that passing `Value::Tag("None")` to `Option.map` immediately returns `None`, but passing `Value::Tag("Some", inner)` executes the closure.

### Days 5-7: The IO API Shell

- **Deliverable 4: IO Built-ins.** Implement `IO.print`, `IO.println`, `IO.readLine`.
- **Deliverable 5: Effect Wrapping.** Since our language separates Pure/Impure code, your IO functions shouldn't just run—they should return a `Value::Effect` wrapper that the runtime will execute later.
- **TDD Focus:** Write tests asserting that calling `io_print.execute(&[Value::String("hello")])` does _not_ print to stdout, but instead returns a `Value::Effect` describing the intent to print.
