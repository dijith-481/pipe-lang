/// Intermediate representation types for the pipe-lang compiler.
///
/// IR is a flat, SSA-like representation that maps directly to
/// Cranelift instructions for JIT compilation.
use ast::SmolStr;

/// A unique identifier for a computed value within a function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub u32);

/// A unique identifier for a basic block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

/// A single SSA instruction.
#[derive(Debug, Clone)]
pub enum Instruction {
    /// Load a constant integer.
    ConstInt(i64),
    /// Load a constant float.
    ConstFloat(f64),
    /// Load a constant boolean.
    ConstBool(bool),
    /// Integer addition.
    Add(ValueId, ValueId),
    /// Integer subtraction.
    Sub(ValueId, ValueId),
    /// Integer multiplication.
    Mul(ValueId, ValueId),
    /// Integer division.
    Div(ValueId, ValueId),
    /// Call a function by name with arguments.
    Call(SmolStr, Vec<ValueId>),
    /// Return a value from the function.
    Return(ValueId),
}

/// A basic block: a sequence of instructions ending with a terminator.
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BlockId,
    pub instructions: Vec<(ValueId, Instruction)>,
    pub terminator: Terminator,
}

/// How a basic block ends.
#[derive(Debug, Clone)]
pub enum Terminator {
    /// Return from the function.
    Return(ValueId),
    /// Unconditional jump to a block.
    Branch(BlockId),
    /// Conditional branch.
    CondBranch {
        condition: ValueId,
        true_block: BlockId,
        false_block: BlockId,
    },
}

/// A function in IR form.
#[derive(Debug, Clone)]
pub struct IrFunction {
    pub name: SmolStr,
    pub params: Vec<(ValueId, SmolStr)>,
    pub blocks: Vec<BasicBlock>,
    pub next_value_id: u32,
    pub next_block_id: u32,
}

impl IrFunction {
    /// Creates a new empty IR function.
    pub fn new(name: SmolStr) -> Self {
        Self {
            name,
            params: Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ir_function_new_has_zero_ids() {
        let func = IrFunction::new("test".into());
        assert_eq!(func.next_value_id, 0);
        assert_eq!(func.next_block_id, 0);
        assert_eq!(func.name.as_str(), "test");
    }

    #[test]
    fn ir_function_alloc_value_increments() {
        let mut func = IrFunction::new("test".into());
        let v1 = func.alloc_value();
        let v2 = func.alloc_value();
        assert_eq!(v1, ValueId(0));
        assert_eq!(v2, ValueId(1));
        assert_eq!(func.next_value_id, 2);
    }

    #[test]
    fn ir_function_alloc_block_increments() {
        let mut func = IrFunction::new("test".into());
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
}
