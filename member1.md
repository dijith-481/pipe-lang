# Member 1 — The Backend (Cranelift JIT Compiler)

**Crate Ownership:** `crates/runtime/src/jit.rs`
**Mission:** You own the final stage of the compiler pipeline. Your job is to consume the flat Intermediate Representation (IR) produced by the frontend and translate it into optimized, native machine code using the Cranelift JIT compiler.

You do not need to parse strings, analyze types, or understand closures. By the time data reaches your crate, it is a guaranteed-valid, flat, SSA-lite list of instructions.

## 1. Core Architecture

The frontend provides an `IrModule` containing `IrFunction`s. Every function consists of `BasicBlock`s. Every block contains a linear sequence of `Instruction`s and ends with a single `Terminator`.

Your JIT compiler will:
1. Initialize a `cranelift_jit::JITModule`.
2. Iterate through each `IrFunction`.
3. Create a `cranelift_frontend::FunctionBuilder`.
4. Map `ValueId`s to Cranelift `Value`s using a side-table (`HashMap<ValueId, cranelift::ir::Value>`).
5. Emit native machine code.
6. Return a callable function pointer.

## 2. Cranelift ABI & Value Representation

The language relies on a uniform `Value` enum defined by Member 2. Because Cranelift operates on native machine registers, you will treat the `Value` enum as an opaque data structure (a pointer or packed struct) at the JIT boundary.

*   **Function Signature:** Every JIT-compiled function will use the standard C ABI, taking an array/pointer of arguments and returning a `Value`.
*   **Signature Mapping:** 
    ```rust
    // extern "C" fn(args: *const Value) -> Value
    ```

## 3. Phase 1: JIT Skeleton & Basic Blocks (Days 1–5)

### 1. The Compiler Initialization
Implement the `JitCompiler` struct. Configure Cranelift to use the host architecture's native ISA without PIC (Position Independent Code) overhead, as we execute in memory.

```rust
pub struct JitCompiler {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    module: JITModule,
}

impl JitCompiler {
    pub fn new() -> Self;
    pub fn compile_module(&mut self, ir: &IrModule) -> Result<(), JitError>;
    pub fn get_main(&mut self) -> Result<extern "C" fn() -> Value, JitError>;
}
```

### 2. Block Translation
Map `IrBlock`s to Cranelift blocks. 
*   Iterate over the blocks in the `IrFunction`.
*   Call `builder.create_block()` for each.
*   Process the `Terminator` of each block (`Jump`, `Branch`, `Switch`, `Return`).
*   Wire Cranelift `brz` (branch zero) and `brnz` (branch non-zero) for conditional branching.

## 4. Phase 2: Instruction Translation (Days 6–10)

Map the `Instruction` enum to `builder.ins()`.

### 1. Constants & Primitives
*   `ConstI32(val)` -> `builder.ins().iconst(types::I32, val as i64)`
*   `ConstF64(val)` -> `builder.ins().f64const(val)`
*   `ConstBool(val)` -> `builder.ins().bconst(types::B1, val)`

### 2. Arithmetic & Logic
Map IR instructions like `Add`, `Sub`, `Eq` directly to Cranelift's `iadd`, `isub`, `icmp`. Ensure you read the type from the IR to choose the correct Cranelift opcode (e.g., `fadd` for floats).

### 3. FFI Calls (`Call` Instruction)
When you encounter a `Call` instruction targeting a standard library function, you must invoke a native Rust function.
*   Import the address of the Rust function into Cranelift using `module.declare_function`.
*   Pack the arguments into a continuous array.
*   Emit a Cranelift `call` instruction.

## 5. Phase 3: Heap Types & Closures (Days 11–14)

You are responsible for executing the memory allocation instructions emitted by the frontend.
*   `MakeClosure(ClosureData)`: Call out to a Rust runtime allocation function to create the `Value::Closure` struct, passing the JIT function pointer and the captured variables.
*   `RecordAlloc` / `TagAlloc`: Emit calls to runtime helper functions that construct the `Arc`-backed `Value::Record` and `Value::Tag` structs.

## 6. Testing Requirements
*   `test_jit_arithmetic`: Compile a module that adds two constants and returns the result. Verify the returned function pointer yields the correct value.
*   `test_jit_branching`: Compile a conditional branch and verify both paths execute correctly depending on inputs.
