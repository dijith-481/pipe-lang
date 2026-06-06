//! Intermediate representation for the pipe-lang compiler.
//!
//! IR is a flat, SSA-like representation. The plan is for the AST-to-IR
//! lowering pass (Track A) to produce `IrModule`s, and the Cranelift
//! backend (Track B) to consume them and emit native code.
//!
//! Design goals:
//! 1. **Explicit types.** Every `ValueId` has a known `IrType` so the
//!    Cranelift codegen does not need to re-invent HM.
//! 2. **No implicit boxing.** Aggregates (arrays, records, tags, closures)
//!    are represented by fat pointers `(ptr, len)`; the runtime layout
//!    is decided in one place (see `ir::layout`).
//! 3. **Cranelift-friendly terminators.** Blocks end in `Return`, `Jump`,
//!    `Branch`, or `Switch`. `Switch` is used for pattern matching on
//!    tags.
//! 4. **First-class closures.** `MakeClosure` + `CallIndirect` is the
//!    only function-call shape; named builtins are reached through
//!    `CallNamed`.
//! 5. **Effects as data.** A `do` block is a single `EffectBind` chain.
//!    The runtime executes it left-to-right.
//!
//! The IR is **not** validated here; validation is the lowerer's job.

use std::fmt;

use ast::SmolStr;

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// A unique identifier for a computed value within a function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ValueId(pub u32);

impl fmt::Display for ValueId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.0)
    }
}

/// A unique identifier for a basic block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u32);

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Types in the IR
// ---------------------------------------------------------------------------

/// The type of an IR value.
///
/// Every `ValueId` produced by a lowering pass must have a known
/// `IrType`. The codegen uses this directly to pick Cranelift
/// lane types.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IrType {
    // -- Signed integers --
    I8,
    I16,
    I32,
    I64,
    // -- Unsigned integers --
    U8,
    U16,
    U32,
    U64,
    Usize,
    // -- Floats --
    F32,
    F64,
    // -- Other primitives --
    Bool,
    Str,
    Unit,
    // -- Compound --
    Array(Box<IrType>),
    Record(RecordType),
    Func(FuncType),
    Closure(Box<FuncType>),
    Tag(TagType),
    /// An effectful computation producing a value of the inner type.
    /// At runtime this is a fat pointer; the codegen does not see the
    /// `Effect` distinction (it is erased by the lowerer when sequencing
    /// `do` blocks).
    Effect(Box<IrType>),
}

/// The type of a record: a name (for diagnostics) and an ordered
/// list of field name + type pairs. Order matters for layout.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordType {
    pub name: SmolStr,
    pub fields: Vec<(SmolStr, IrType)>,
}

/// The type of a function: parameter types and a return type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FuncType {
    pub params: Vec<IrType>,
    pub ret: Box<IrType>,
}

/// The type of a tagged union: a name and a list of constructor
/// arities + payload types.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TagType {
    pub name: SmolStr,
    pub variants: Vec<TagVariant>,
}

/// One variant of a tagged union. `discriminant` is the runtime
/// integer tag (0, 1, 2, ...).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TagVariant {
    pub name: SmolStr,
    pub discriminant: u32,
    pub payload: Vec<IrType>,
}

impl IrType {
    /// Returns true if this is a numeric type (any width/signedness).
    #[must_use]
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            IrType::I8
                | IrType::I16
                | IrType::I32
                | IrType::I64
                | IrType::U8
                | IrType::U16
                | IrType::U32
                | IrType::U64
                | IrType::Usize
                | IrType::F32
                | IrType::F64
        )
    }

    /// Returns true if this is a pointer-shaped type (array, record,
    /// tag, closure, str). These are passed as fat pointers.
    #[must_use]
    pub fn is_heap(&self) -> bool {
        matches!(
            self,
            IrType::Array(_)
                | IrType::Record(_)
                | IrType::Closure(_)
                | IrType::Tag(_)
                | IrType::Str
        )
    }
}

impl fmt::Display for IrType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrType::I8 => write!(f, "i8"),
            IrType::I16 => write!(f, "i16"),
            IrType::I32 => write!(f, "i32"),
            IrType::I64 => write!(f, "i64"),
            IrType::U8 => write!(f, "u8"),
            IrType::U16 => write!(f, "u16"),
            IrType::U32 => write!(f, "u32"),
            IrType::U64 => write!(f, "u64"),
            IrType::Usize => write!(f, "usize"),
            IrType::F32 => write!(f, "f32"),
            IrType::F64 => write!(f, "f64"),
            IrType::Bool => write!(f, "bool"),
            IrType::Str => write!(f, "str"),
            IrType::Unit => write!(f, "()"),
            IrType::Array(inner) => write!(f, "Array<{inner}>"),
            IrType::Record(r) => {
                write!(f, "{}", r.name)?;
                write!(f, "{{")?;
                for (i, (n, t)) in r.fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{n}: {t}")?;
                }
                write!(f, "}}")
            }
            IrType::Func(ft) => write_func_type(f, ft),
            IrType::Closure(ft) => write_func_type(f, ft),
            IrType::Tag(t) => {
                write!(f, "{}", t.name)
            }
            IrType::Effect(inner) => write!(f, "Effect<{inner}>"),
        }
    }
}

fn write_func_type(f: &mut fmt::Formatter<'_>, ft: &FuncType) -> fmt::Result {
    write!(f, "(")?;
    for (i, p) in ft.params.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }
        write!(f, "{p}")?;
    }
    write!(f, ") -> {}", ft.ret)
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

/// A single SSA instruction.
///
/// Instructions are *pure* (no terminator, no control flow) except
/// for `Call` and `CallIndirect` which may diverge (effects).
#[derive(Debug, Clone)]
pub enum Instruction {
    // -- Constants --
    ConstI8(i8),
    ConstI16(i16),
    ConstI32(i32),
    ConstI64(i64),
    ConstU8(u8),
    ConstU16(u16),
    ConstU32(u32),
    ConstU64(u64),
    ConstUsize(usize),
    ConstF32(f32),
    ConstF64(f64),
    ConstBool(bool),
    ConstStr(SmolStr),
    ConstUnit,

    // -- Arithmetic --
    Add(ValueId, ValueId),
    Sub(ValueId, ValueId),
    Mul(ValueId, ValueId),
    Div(ValueId, ValueId),
    Rem(ValueId, ValueId),
    Neg(ValueId),

    // -- Comparison (returns Bool) --
    Eq(ValueId, ValueId),
    Ne(ValueId, ValueId),
    Lt(ValueId, ValueId),
    Le(ValueId, ValueId),
    Gt(ValueId, ValueId),
    Ge(ValueId, ValueId),

    // -- Logical --
    And(ValueId, ValueId),
    Or(ValueId, ValueId),
    Not(ValueId),

    // -- Arrays --
    /// Allocate an array of the given length, filled with `init`.
    /// The init value is type-checked to match the element type.
    ArrayAlloc {
        len: ValueId,
        init: ValueId,
    },
    /// Read an element at `index`. Lowerer inserts bounds checks (panics
    /// turn into `Panic` instructions; the codegen lowers those to
    /// calls into the runtime's `trap` function).
    ArrayGet {
        array: ValueId,
        index: ValueId,
    },
    /// Write `value` to `array[index]`. Returns Unit.
    ArraySet {
        array: ValueId,
        index: ValueId,
        value: ValueId,
    },
    /// Length of an array. Returns Usize.
    ArrayLen(ValueId),
    /// Concatenate two arrays. Returns a new array.
    ArrayConcat(ValueId, ValueId),

    // -- Records --
    /// Allocate a record with the given fields (in declaration order).
    RecordAlloc {
        type_name: SmolStr,
        fields: Vec<ValueId>,
    },
    /// Read a field by name.
    RecordGet {
        record: ValueId,
        field: SmolStr,
        field_index: u32,
    },
    /// Write a field. Returns the new record (records are immutable;
    /// this allocates a fresh copy and updates only the named field).
    RecordSet {
        record: ValueId,
        field: SmolStr,
        field_index: u32,
        value: ValueId,
    },

    // -- Tags (sum types) --
    /// Construct a tag variant. The discriminant is resolved at
    /// lower time from the type's variant table.
    TagConstruct {
        type_name: SmolStr,
        variant: SmolStr,
        discriminant: u32,
        payload: Vec<ValueId>,
    },
    /// Extract the discriminant of a tag value. Returns U32.
    TagDiscriminant(ValueId),
    /// Extract the `index`-th payload field. Returns the field's
    /// declared type.
    TagGet {
        value: ValueId,
        index: u32,
    },

    // -- Closures --
    /// Wrap a function pointer plus its captured environment into a
    /// closure value. The `func_name` references an `IrFunction` in the
    /// same module; the codegen resolves it to a native pointer.
    MakeClosure {
        func_name: SmolStr,
        captures: Vec<ValueId>,
    },
    /// Call a closure value with the given arguments. Used after
    /// `MakeClosure`, or when the callee is a function parameter.
    CallIndirect {
        callee: ValueId,
        args: Vec<ValueId>,
    },

    // -- Named calls (builtins, top-level functions) --
    /// Call a named function. The codegen resolves the name to a
    /// builtin (registered in `runtime::bridge`) or to another
    /// `IrFunction` in the same module.
    CallNamed {
        name: SmolStr,
        args: Vec<ValueId>,
    },

    // -- Effects --
    /// Sequentially bind an effect to a name, then run `next` with
    /// the bound value. Lowered from `do { x <- m; rest }`.
    /// At runtime this is just a tail call to `m`, then a tail call
    /// to a closure that runs `rest` with the captured result.
    EffectBind {
        effect: ValueId,
        continuation: ValueId,
    },
    /// Construct an effect value (a `builtin + args` pair) without
    /// running it yet. The codegen wraps this in a Cranelift
    /// function pointer.
    EffectValue {
        builtin: SmolStr,
        args: Vec<ValueId>,
    },
    /// Pure: do nothing, return Unit. Used at the end of an
    /// expression-only `do` block.
    EffectReturn(ValueId),

    // -- Strings --
    /// Concatenate a sequence of string fragments (from a template
    /// literal). Each fragment is either a `ConstStr` value or any
    /// value with a `Display` impl. The codegen calls
    /// `runtime::stdlib::str::concat`.
    StrConcat {
        parts: Vec<ValueId>,
    },
    /// A `println(arg)` call. Sugar for
    /// `Effect::bind(EffectValue("IO.println", [arg]), Effect::pure_unit)`.
    Println(ValueId),

    // -- Panic (for bounds checks, non-exhaustive match, etc.) --
    /// Trap with a message. Codegen lowers to `cranelift::codegen::ir::TrapCode::UnreachableCodeReachable`.
    Panic {
        msg: SmolStr,
    },
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instruction::ConstI8(v) => write!(f, "const.i8 {v}"),
            Instruction::ConstI16(v) => write!(f, "const.i16 {v}"),
            Instruction::ConstI32(v) => write!(f, "const.i32 {v}"),
            Instruction::ConstI64(v) => write!(f, "const.i64 {v}"),
            Instruction::ConstU8(v) => write!(f, "const.u8 {v}"),
            Instruction::ConstU16(v) => write!(f, "const.u16 {v}"),
            Instruction::ConstU32(v) => write!(f, "const.u32 {v}"),
            Instruction::ConstU64(v) => write!(f, "const.u64 {v}"),
            Instruction::ConstUsize(v) => write!(f, "const.usize {v}"),
            Instruction::ConstF32(v) => write!(f, "const.f32 {v}"),
            Instruction::ConstF64(v) => write!(f, "const.f64 {v}"),
            Instruction::ConstBool(v) => write!(f, "const.bool {v}"),
            Instruction::ConstStr(s) => write!(f, "const.str {s:?}"),
            Instruction::ConstUnit => write!(f, "const.unit"),
            Instruction::Add(a, b) => write!(f, "{a} + {b}"),
            Instruction::Sub(a, b) => write!(f, "{a} - {b}"),
            Instruction::Mul(a, b) => write!(f, "{a} * {b}"),
            Instruction::Div(a, b) => write!(f, "{a} / {b}"),
            Instruction::Rem(a, b) => write!(f, "{a} % {b}"),
            Instruction::Neg(a) => write!(f, "-{a}"),
            Instruction::Eq(a, b) => write!(f, "{a} == {b}"),
            Instruction::Ne(a, b) => write!(f, "{a} != {b}"),
            Instruction::Lt(a, b) => write!(f, "{a} < {b}"),
            Instruction::Le(a, b) => write!(f, "{a} <= {b}"),
            Instruction::Gt(a, b) => write!(f, "{a} > {b}"),
            Instruction::Ge(a, b) => write!(f, "{a} >= {b}"),
            Instruction::And(a, b) => write!(f, "{a} && {b}"),
            Instruction::Or(a, b) => write!(f, "{a} || {b}"),
            Instruction::Not(a) => write!(f, "!{a}"),
            Instruction::ArrayAlloc { len, init } => write!(f, "array_alloc {len} {init}"),
            Instruction::ArrayGet { array, index } => write!(f, "array_get {array}[{index}]"),
            Instruction::ArraySet {
                array,
                index,
                value,
            } => write!(f, "array_set {array}[{index}] = {value}"),
            Instruction::ArrayLen(a) => write!(f, "array_len {a}"),
            Instruction::ArrayConcat(a, b) => write!(f, "array_concat {a} {b}"),
            Instruction::RecordAlloc { type_name, fields } => {
                write!(f, "record_alloc {type_name}(")?;
                for (i, v) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            Instruction::RecordGet {
                record,
                field,
                field_index: _,
            } => write!(f, "record_get {record}.{field}"),
            Instruction::RecordSet {
                record,
                field,
                field_index: _,
                value,
            } => write!(f, "record_set {record}.{field} = {value}"),
            Instruction::TagConstruct {
                type_name,
                variant,
                discriminant: _,
                payload,
            } => {
                write!(f, "tag {type_name}::{variant}(")?;
                for (i, v) in payload.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            Instruction::TagDiscriminant(v) => write!(f, "tag_discriminant {v}"),
            Instruction::TagGet { value, index } => write!(f, "tag_get {value}.{index}"),
            Instruction::MakeClosure {
                func_name,
                captures,
            } => {
                write!(f, "closure {func_name}(")?;
                for (i, v) in captures.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            Instruction::CallIndirect { callee, args } => {
                write!(f, "call_indirect {callee}(")?;
                for (i, v) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            Instruction::CallNamed { name, args } => {
                write!(f, "call {name}(")?;
                for (i, v) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            Instruction::EffectBind {
                effect,
                continuation,
            } => write!(f, "effect_bind {effect} -> {continuation}"),
            Instruction::EffectValue { builtin, args } => {
                write!(f, "effect_value {builtin}(")?;
                for (i, v) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            Instruction::EffectReturn(v) => write!(f, "effect_return {v}"),
            Instruction::StrConcat { parts } => {
                write!(f, "str_concat(")?;
                for (i, v) in parts.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            Instruction::Println(v) => write!(f, "println {v}"),
            Instruction::Panic { msg } => write!(f, "panic {msg:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Terminators
// ---------------------------------------------------------------------------

/// How a basic block ends.
#[derive(Debug, Clone)]
pub enum Terminator {
    /// Return a value from the function.
    Return(ValueId),
    /// Unconditional jump to a block with arguments.
    Jump { target: BlockId, args: Vec<ValueId> },
    /// Conditional branch.
    Branch {
        condition: ValueId,
        then_block: BlockId,
        then_args: Vec<ValueId>,
        else_block: BlockId,
        else_args: Vec<ValueId>,
    },
    /// N-way branch on a tag discriminant. Used for `match` on sum types.
    /// `default` is the fallthrough (e.g. wildcard arm).
    Switch {
        discriminant: ValueId,
        /// (discriminant_value, target_block, args)
        arms: Vec<(u32, BlockId, Vec<ValueId>)>,
        default: Option<(BlockId, Vec<ValueId>)>,
    },
    /// Tail call. Used for `TailCall` rewriting of self-recursive
    /// functions (e.g. `quicksort`). Codegen emits a jump rather
    /// than a `call` to avoid stack growth.
    TailCall { callee: ValueId, args: Vec<ValueId> },
    /// Unreachable. Inserted after a `Panic` to satisfy Cranelift.
    Unreachable,
}

impl fmt::Display for Terminator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Terminator::Return(v) => write!(f, "ret {v}"),
            Terminator::Jump { target, args } => {
                write!(f, "jump {target}(")?;
                for (i, v) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            Terminator::Branch {
                condition,
                then_block,
                else_block,
                ..
            } => write!(f, "br {condition}, {then_block}, {else_block}"),
            Terminator::Switch {
                discriminant,
                arms,
                default,
            } => {
                write!(f, "switch {discriminant} {{ ")?;
                for (d, t, _) in arms {
                    write!(f, "{d} -> {t}, ")?;
                }
                if let Some((t, _)) = default {
                    write!(f, "default -> {t} ")?;
                }
                write!(f, "}}")
            }
            Terminator::TailCall { callee, args } => {
                write!(f, "tail_call {callee}(")?;
                for (i, v) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            Terminator::Unreachable => write!(f, "unreachable"),
        }
    }
}

// ---------------------------------------------------------------------------
// Basic blocks and functions
// ---------------------------------------------------------------------------

/// A basic block: a sequence of instructions ending with a terminator.
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BlockId,
    /// Block parameters (SSA block arguments / Phi nodes).
    pub params: Vec<(ValueId, IrType)>,
    /// Instructions in this block. Each instruction is paired with
    /// the `ValueId` it defines (or `None` for value-less instructions
    /// like `Println`).
    pub instructions: Vec<(Option<ValueId>, Instruction)>,
    pub terminator: Terminator,
}

impl BasicBlock {
    /// Creates a new empty block with the given ID and no params.
    #[must_use]
    pub fn new(id: BlockId) -> Self {
        Self {
            id,
            params: Vec::new(),
            instructions: Vec::new(),
            terminator: Terminator::Unreachable,
        }
    }
}

/// A function in IR form.
#[derive(Debug, Clone)]
pub struct IrFunction {
    pub name: SmolStr,
    /// Function parameters, with their declared types.
    pub params: Vec<(ValueId, SmolStr, IrType)>,
    /// The function's return type.
    pub return_type: IrType,
    /// All blocks in the function. Block 0 is the entry block.
    pub blocks: Vec<BasicBlock>,
    /// Monotonically increasing counter for allocating `ValueId`s.
    pub next_value_id: u32,
    /// Monotonically increasing counter for allocating `BlockId`s.
    pub next_block_id: u32,
}

impl IrFunction {
    /// Creates a new empty IR function with the given return type.
    pub fn new(name: SmolStr, return_type: IrType) -> Self {
        Self {
            name,
            params: Vec::new(),
            return_type,
            blocks: Vec::new(),
            next_value_id: 0,
            next_block_id: 0,
        }
    }

    /// Allocates a new unique value ID.
    pub fn alloc_value(&mut self) -> ValueId {
        let id = ValueId(self.next_value_id);
        self.next_value_id += 1;
        id
    }

    /// Allocates a new unique block ID.
    pub fn alloc_block(&mut self) -> BlockId {
        let id = BlockId(self.next_block_id);
        self.next_block_id += 1;
        id
    }
}

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

/// A top-level declaration after lowering. For 0.1 we only support
/// `Function` and `TypeAlias`; full constant folding is out of scope.
#[derive(Debug, Clone)]
pub enum IrDecl {
    /// A function definition.
    Function(IrFunction),
    /// A type alias (e.g. `type AppState = | Idle | Loading | ...`).
    /// The codegen uses these to resolve tag discriminants.
    TypeAlias { name: SmolStr, ty: IrType },
}

/// A complete IR module: everything needed to compile one source file.
#[derive(Debug, Clone, Default)]
pub struct IrModule {
    /// Imported module names (e.g. `"stdlib::io"`).
    pub imports: Vec<SmolStr>,
    /// Top-level declarations.
    pub decls: Vec<IrDecl>,
}

impl IrModule {
    /// Creates an empty module.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns an iterator over all functions in this module.
    pub fn functions(&self) -> impl Iterator<Item = &IrFunction> + '_ {
        self.decls.iter().filter_map(|d| match d {
            IrDecl::Function(f) => Some(f),
            IrDecl::TypeAlias { .. } => None,
        })
    }

    /// Returns a mutable slice of all functions in this module.
    pub fn functions_mut(&mut self) -> impl Iterator<Item = &mut IrFunction> {
        self.decls.iter_mut().filter_map(|d| match d {
            IrDecl::Function(f) => Some(f),
            IrDecl::TypeAlias { .. } => None,
        })
    }

    /// Looks up a function by name. Returns `None` if not found.
    #[must_use]
    pub fn function(&self, name: &str) -> Option<&IrFunction> {
        self.functions().find(|f| f.name.as_str() == name)
    }
}

impl fmt::Display for IrModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for imp in &self.imports {
            writeln!(f, "use {imp}")?;
        }
        for decl in &self.decls {
            match decl {
                IrDecl::Function(func) => {
                    writeln!(f, "fn {}(", func.name)?;
                    for (i, (_, name, ty)) in func.params.iter().enumerate() {
                        if i > 0 {
                            writeln!(f, ", ")?;
                        }
                        write!(f, "  {name}: {ty}")?;
                    }
                    writeln!(f, ") -> {} {{", func.return_type)?;
                    for block in &func.blocks {
                        writeln!(f, "  bb{}(", block.id.0)?;
                        for (i, (vid, ty)) in block.params.iter().enumerate() {
                            if i > 0 {
                                writeln!(f, ", ")?;
                            }
                            write!(f, "    {vid}: {ty}")?;
                        }
                        writeln!(f, "  ):")?;
                        for (vid, inst) in &block.instructions {
                            if let Some(v) = vid {
                                writeln!(f, "    {v} = {inst}")?;
                            } else {
                                writeln!(f, "    {inst}")?;
                            }
                        }
                        writeln!(f, "    {}", block.terminator)?;
                    }
                    writeln!(f, "}}")?;
                }
                IrDecl::TypeAlias { name, ty } => {
                    writeln!(f, "type {name} = {ty}")?;
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ir_function_new_has_zero_ids() {
        let func = IrFunction::new("test".into(), IrType::Unit);
        assert_eq!(func.next_value_id, 0);
        assert_eq!(func.next_block_id, 0);
        assert_eq!(func.name.as_str(), "test");
    }

    #[test]
    fn ir_function_alloc_value_increments() {
        let mut func = IrFunction::new("test".into(), IrType::Unit);
        let v1 = func.alloc_value();
        let v2 = func.alloc_value();
        assert_eq!(v1, ValueId(0));
        assert_eq!(v2, ValueId(1));
        assert_eq!(func.next_value_id, 2);
    }

    #[test]
    fn ir_function_alloc_block_increments() {
        let mut func = IrFunction::new("test".into(), IrType::Unit);
        let b1 = func.alloc_block();
        let b2 = func.alloc_block();
        assert_eq!(b1, BlockId(0));
        assert_eq!(b2, BlockId(1));
        assert_eq!(func.next_block_id, 2);
    }

    #[test]
    fn instruction_debug_format() {
        let instr = Instruction::Add(ValueId(0), ValueId(1));
        assert!(format!("{instr:?}").contains("Add"));
    }

    #[test]
    fn terminator_debug_format() {
        let term = Terminator::Return(ValueId(0));
        assert!(format!("{term:?}").contains("Return"));
    }

    #[test]
    fn ir_type_is_numeric() {
        assert!(IrType::I32.is_numeric());
        assert!(IrType::F64.is_numeric());
        assert!(IrType::U8.is_numeric());
        assert!(!IrType::Bool.is_numeric());
        assert!(!IrType::Str.is_numeric());
    }

    #[test]
    fn ir_type_is_heap() {
        assert!(IrType::Str.is_heap());
        assert!(IrType::Array(Box::new(IrType::I32)).is_heap());
        assert!(!IrType::I32.is_heap());
        assert!(!IrType::Bool.is_heap());
    }

    #[test]
    fn ir_type_display() {
        assert_eq!(IrType::I32.to_string(), "i32");
        assert_eq!(IrType::Str.to_string(), "str");
        assert_eq!(IrType::Unit.to_string(), "()");
        assert_eq!(
            IrType::Array(Box::new(IrType::I32)).to_string(),
            "Array<i32>"
        );
    }

    #[test]
    fn ir_module_function_lookup() {
        let mut module = IrModule::new();
        module.decls.push(IrDecl::Function(IrFunction::new(
            "main".into(),
            IrType::Unit,
        )));
        assert!(module.function("main").is_some());
        assert!(module.function("missing").is_none());
    }

    #[test]
    fn ir_module_display_round_trip() {
        let mut module = IrModule::new();
        module.decls.push(IrDecl::TypeAlias {
            name: "MyInt".into(),
            ty: IrType::I32,
        });
        let s = module.to_string();
        assert!(s.contains("type MyInt = i32"));
    }

    #[test]
    fn instruction_display_includes_value_ids() {
        let instr = Instruction::Add(ValueId(7), ValueId(8));
        let s = instr.to_string();
        assert!(s.contains("v7"));
        assert!(s.contains("v8"));
    }

    #[test]
    fn tag_type_constructs() {
        let ty = TagType {
            name: "Option".into(),
            variants: vec![
                TagVariant {
                    name: "None".into(),
                    discriminant: 0,
                    payload: vec![],
                },
                TagVariant {
                    name: "Some".into(),
                    discriminant: 1,
                    payload: vec![IrType::I32],
                },
            ],
        };
        assert_eq!(ty.variants.len(), 2);
        assert_eq!(ty.variants[1].discriminant, 1);
    }

    #[test]
    fn record_type_field_order_preserved() {
        let ty = RecordType {
            name: "Person".into(),
            fields: vec![("name".into(), IrType::Str), ("age".into(), IrType::I32)],
        };
        assert_eq!(ty.fields[0].0.as_str(), "name");
        assert_eq!(ty.fields[1].0.as_str(), "age");
    }
}
