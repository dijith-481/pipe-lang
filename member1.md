# Member 1: The Type Checker (Week 1 Deliverables)

**Crate Ownership:** `crates/typechecker`
**Mission:** Build a robust Hindley-Milner (HM) type inference engine. For Week 1, you will focus on pure inference, environment management, and unification without worrying about parsing.

## The Workflow & TDD Strategy

Until Day 5, the Parser won't be ready. **This is intentional.** You will do pure TDD by manually constructing AST nodes using the `bumpalo` arena the Lead Architect provides on Day 1, passing them to your type checker, and asserting the output types.

### Your API Contract (Provided by Lead on Day 1)

You will consume the `Expr<'a>` and `Decl<'a>` enums from the `crates/ast` crate. You are responsible for defining the `Type` enum and implementing the inference logic.

## Week 1 Deliverables & Timeline

### Days 1-2: Environments and Primitives

- **Deliverable 1: The `Type` Enum.** Define `TypeId`, `MonoType`, and `PolyType` (for generics).
- **Deliverable 2: The Type Environment.** Build a scoped environment (symbol table) that allows pushing and popping scopes for nested functions.
- **TDD Focus:** Write tests that create a new environment, insert a variable `x : Int`, push a scope, insert `y : Float`, and verify `lookup("x")` and `lookup("y")` resolve correctly, while popping removes `y`.

### Days 3-4: Unification & Basic Inference

- **Deliverable 3: The Unification Algorithm.** Write the logic that solves equations like `T1 = Int -> T2`.
- **Deliverable 4: Primitive Inference.** Implement `infer_expr` for constants, identifiers, and let-bindings.
- **TDD Focus:** Construct an AST like `let x = 5` manually in Rust. Pass it to `infer_expr` and assert it returns `Result::Ok(Type::Int)`. Test mismatch failures (e.g., trying to unify `Int` and `Float`).

### Days 5-7: Functions & Parser Integration

- **Deliverable 5: Function Inference.** Implement inference for Function Application (`f(x)`) and Lambdas.
- **Deliverable 6: Parser Integration.** (On Day 5, the Lead will hand you the working Parser). Replace your manual AST construction in tests with: `let ast = parse("add = (x) => x + 1"); check_types(&ast);`
- **TDD Focus:** Write end-to-end type check tests using real strings of code. Write tests that _fail_ with beautifully constructed `TypeError` variants.
