use bumpalo::Bump;
use ir::{Instruction, IrModule, IrType, Terminator, lower};
use parser::parse;
use typechecker::typecheck;

fn lower_src(src: &str) -> IrModule {
    let bump = Bump::new();
    let prog = parse(src, &bump).expect("parse failed");
    let typed = typecheck(&prog).expect("typecheck failed");
    lower(&typed).expect("lower failed")
}

// ---------------------------------------------------------------------------
// Basic lowering
// ---------------------------------------------------------------------------

#[test]
fn lower_const_i32_binding() {
    // `let x = 42` → one function `x` returning i32 constant
    let m = lower_src("let x = 42");
    let f = m.function("x").expect("function x not found");
    assert_eq!(f.return_type, IrType::I32);
    // entry block must end in Return
    let entry = &f.blocks[0];
    assert!(matches!(entry.terminator, Terminator::Return(_)));
}

#[test]
fn lower_bool_binding() {
    let m = lower_src("let flag = true");
    let f = m.function("flag").unwrap();
    assert_eq!(f.return_type, IrType::Bool);
}

#[test]
fn lower_str_binding() {
    let m = lower_src(r#"let greeting = "hello""#);
    let f = m.function("greeting").unwrap();
    assert_eq!(f.return_type, IrType::Str);
}

#[test]
fn lower_f64_binding() {
    let m = lower_src("let pi = 3.14");
    let f = m.function("pi").unwrap();
    assert_eq!(f.return_type, IrType::F64);
}

// ---------------------------------------------------------------------------
// Binary operations
// ---------------------------------------------------------------------------

#[test]
fn lower_add_i32() {
    let m = lower_src("let sum = 1 + 2");
    let f = m.function("sum").unwrap();
    assert_eq!(f.return_type, IrType::I32);
    let entry = &f.blocks[0];
    // Should have at least: const 1, const 2, add
    assert!(entry.instructions.len() >= 3);
}

#[test]
fn lower_comparison_returns_bool() {
    let m = lower_src("let gt = 5 > 3");
    let f = m.function("gt").unwrap();
    assert_eq!(f.return_type, IrType::Bool);
}

// ---------------------------------------------------------------------------
// Lambdas
// ---------------------------------------------------------------------------

#[test]
fn lower_identity_lambda() {
    // `let id = (x) => x` — one param, returns same value
    let m = lower_src("let id = (x) => x");
    let f = m.function("id").unwrap();
    assert!(!f.params.is_empty());
    let entry = &f.blocks[0];
    assert!(matches!(entry.terminator, Terminator::Return(_)));
}

#[test]
fn lower_add_lambda() {
    let m = lower_src("let add = (a, b) => a + b");
    let f = m.function("add").unwrap();
    assert_eq!(f.params.len(), 2);
    assert_eq!(f.return_type, IrType::I32);
}

#[test]
fn lower_annotated_lambda() {
    let m = lower_src("let neg: (i32) -> i32 = (x) => x");
    let f = m.function("neg").unwrap();
    assert_eq!(f.return_type, IrType::I32);
}

// ---------------------------------------------------------------------------
// If expression
// ---------------------------------------------------------------------------

#[test]
fn lower_if_expr() {
    let m = lower_src("let abs = (x) => if x > 0 { x } else { 0 }");
    let f = m.function("abs").unwrap();
    // Must have a branch block
    let has_branch = f
        .blocks
        .iter()
        .any(|b| matches!(b.terminator, Terminator::Branch { .. }));
    assert!(has_branch, "expected a Branch terminator");
}

// ---------------------------------------------------------------------------
// Block expression
// ---------------------------------------------------------------------------

#[test]
fn lower_block_expr() {
    let m = lower_src("let calc = (x) => { let y = x + 1 y }");
    let f = m.function("calc").unwrap();
    assert_eq!(f.return_type, IrType::I32);
}

// ---------------------------------------------------------------------------
// Multiple declarations
// ---------------------------------------------------------------------------

#[test]
fn lower_multiple_decls() {
    let m = lower_src("let a = 1\nlet b = 2");
    assert!(m.function("a").is_some());
    assert!(m.function("b").is_some());
}

// ---------------------------------------------------------------------------
// Use declarations
// ---------------------------------------------------------------------------

#[test]
fn lower_use_decl_adds_import() {
    let m = lower_src("use stdlib::io");
    assert!(m.imports.contains(&"stdlib::io".into()));
}

// ---------------------------------------------------------------------------
// Array literal
// ---------------------------------------------------------------------------

#[test]
fn lower_array_literal() {
    let m = lower_src("let arr = [1, 2, 3]");
    let f = m.function("arr").unwrap();
    assert_eq!(f.return_type, IrType::Array(Box::new(IrType::I32)));
}

// ---------------------------------------------------------------------------
// Record literal
// ---------------------------------------------------------------------------

#[test]
fn lower_record_literal() {
    let m = lower_src(r#"let rec = { name: "Alice", age: 30 }"#);
    let f = m.function("rec").unwrap();
    assert!(matches!(f.return_type, IrType::Record(_)));
}

// ---------------------------------------------------------------------------
// Regression tests for the gap fixes
// ---------------------------------------------------------------------------

/// Merge block param for if/else must use the actual inferred type, not I32.
#[test]
fn fix_merge_block_param_type_if() {
    let m = lower_src("let r = if true { 3.14 } else { 2.71 }");
    let f = m.function("r").unwrap();
    // The merge block carries the f64 result as a block parameter.
    let merge = f
        .blocks
        .iter()
        .find(|b| !b.params.is_empty())
        .expect("merge block with param");
    assert_eq!(
        merge.params[0].1,
        IrType::F64,
        "if merge param should be F64, not I32"
    );
}

/// Merge block param for match must use the actual inferred type.
#[test]
fn fix_merge_block_param_type_match() {
    let m = lower_src("let r = (x) => match x { true => 1i64 _ => 0i64 }");
    let f = m.function("r").unwrap();
    let merge = f
        .blocks
        .iter()
        .find(|b| !b.params.is_empty())
        .expect("merge block");
    assert_eq!(
        merge.params[0].1,
        IrType::I64,
        "match merge param should be I64"
    );
}

/// Top-level lambda params must carry the correct IrType from annotations.
#[test]
fn fix_lambda_param_types_from_annotation() {
    let m = lower_src("let neg: (f64) -> f64 = (x) => x");
    let f = m.function("neg").unwrap();
    // First param must be f64, not i32.
    assert_eq!(f.params[0].2, IrType::F64, "annotated param should be F64");
}

/// Lambda params from the inferred Func type must not all be i32.
#[test]
fn fix_lambda_param_types_from_type_map() {
    // add(a,b) where both are i32 — the type map gives Func{[i32,i32]->i32}.
    let m = lower_src("let add = (a: i32, b: i32) => a + b");
    let f = m.function("add").unwrap();
    assert_eq!(f.params[0].2, IrType::I32);
    assert_eq!(f.params[1].2, IrType::I32);
}

/// RecordGet field_index must be the sorted position, not always 0.
#[test]
fn fix_record_field_index() {
    // { age: 30, name: "Alice" } — BTreeMap sorts keys, so age=0, name=1
    let m = lower_src(r#"let get_name = (r: { age: i32, name: str }) => r.name"#);
    let f = m.function("get_name").unwrap();
    let record_get = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .find_map(|(_, inst)| {
            if let Instruction::RecordGet {
                field, field_index, ..
            } = inst
            {
                if field.as_str() == "name" {
                    Some(*field_index)
                } else {
                    None
                }
            } else {
                None
            }
        });
    assert_eq!(
        record_get,
        Some(1),
        "name should be at index 1 (after age in sorted order)"
    );
}

/// TypeAlias decls must produce IrDecl::TypeAlias entries.
#[test]
fn fix_type_alias_lowering() {
    let m = lower_src("type UserId = i64");
    let has_alias = m
        .decls
        .iter()
        .any(|d| matches!(d, ir::IrDecl::TypeAlias { name, .. } if name.as_str() == "UserId"));
    assert!(has_alias, "TypeAlias decl should produce IrDecl::TypeAlias");
}

/// Match discriminants for literal patterns use the literal value, not ordinal position.
#[test]
fn fix_match_literal_discriminant() {
    // match on bool: true arm should get ConstBool(true) or ConstI32(1), not ConstI64(1).
    let m = lower_src("let describe = (b) => match b { true => 1 false => 0 }");
    let f = m.function("describe").unwrap();
    // Primitive match now uses cascading Branch, not Switch.
    // Verify we NEVER emit ConstI64(1) for the true arm — the old bug.
    let has_bad_i64 = f.blocks.iter().any(|b| {
        b.instructions
            .iter()
            .any(|(_, inst)| matches!(inst, Instruction::ConstI64(1)))
    });
    assert!(
        !has_bad_i64,
        "should never emit ConstI64(1) — type dispatch must produce type-appropriate constant"
    );
    // Find the true literal reference — either ConstBool(true) or ConstI32(1).
    let has_correct = f.blocks.iter().any(|b| {
        b.instructions.iter().any(|(_, inst)| {
            matches!(inst, Instruction::ConstBool(true)) || matches!(inst, Instruction::ConstI32(1))
        })
    });
    assert!(
        has_correct,
        "true literal should map to type-appropriate constant"
    );
}

#[test]
fn lower_closure_captures() {
    // `adder` closes over `n`; lowerer must emit MakeClosure or inline
    let m = lower_src("let adder = (n) => (x) => n + x");
    let f = m.function("adder").unwrap();
    // Result must be a Func/Closure type
    assert!(
        matches!(f.return_type, IrType::Closure(_) | IrType::Func(_)),
        "expected Closure or Func return type, got {:?}",
        f.return_type
    );
}

// ---------------------------------------------------------------------------
// Template strings
// ---------------------------------------------------------------------------

#[test]
fn lower_template_string() {
    let m = lower_src(r#"let msg = `hello world`"#);
    let f = m.function("msg").unwrap();
    assert_eq!(f.return_type, IrType::Str);
}

#[test]
fn lower_template_with_interpolation() {
    let m = lower_src("let greet = (name) => `hi ${name}`");
    let f = m.function("greet").unwrap();
    assert_eq!(f.return_type, IrType::Str);
    let entry = &f.blocks[0];
    let has_str_concat = entry
        .instructions
        .iter()
        .any(|(_, inst)| matches!(inst, Instruction::StrConcat { .. }));
    assert!(has_str_concat, "expected StrConcat instruction");
}

// ---------------------------------------------------------------------------
// Index expressions
// ---------------------------------------------------------------------------

#[test]
fn lower_index_array() {
    let m = lower_src("let first = (arr) => arr[0]");
    let f = m.function("first").unwrap();
    let has_array_get = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, inst)| matches!(inst, Instruction::ArrayGet { .. }));
    assert!(has_array_get, "expected ArrayGet instruction");
}

// ---------------------------------------------------------------------------
// Float arithmetic
// ---------------------------------------------------------------------------

#[test]
fn lower_float_add() {
    let m = lower_src("let pi = 3.14 + 2.71");
    let f = m.function("pi").unwrap();
    assert_eq!(f.return_type, IrType::F64);
}

// ---------------------------------------------------------------------------
// Unary operations
// ---------------------------------------------------------------------------

#[test]
fn lower_negate() {
    let m = lower_src("let neg = (x: i32) => -x");
    let f = m.function("neg").unwrap();
    let has_neg = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, inst)| matches!(inst, Instruction::Neg(_)));
    assert!(has_neg, "expected Neg instruction");
}

#[test]
fn lower_not() {
    let m = lower_src("let negate = (b) => !b");
    let f = m.function("negate").unwrap();
    let has_not = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, inst)| matches!(inst, Instruction::Not(_)));
    assert!(has_not, "expected Not instruction");
}

// ---------------------------------------------------------------------------
// Logical operators
// ---------------------------------------------------------------------------

#[test]
fn lower_logical_and() {
    let m = lower_src("let both = (a, b) => a && b");
    let f = m.function("both").unwrap();
    let has_and = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, inst)| matches!(inst, Instruction::And(_, _)));
    assert!(has_and, "expected And instruction");
}

#[test]
fn lower_logical_or() {
    let m = lower_src("let either = (a, b) => a || b");
    let f = m.function("either").unwrap();
    let has_or = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, inst)| matches!(inst, Instruction::Or(_, _)));
    assert!(has_or, "expected Or instruction");
}

// ---------------------------------------------------------------------------
// Nested if/else
// ---------------------------------------------------------------------------

#[test]
fn lower_nested_if() {
    let m = lower_src("let clamp = (x) => if x > 100 { 100 } else { if x < 0 { 0 } else { x } }");
    let f = m.function("clamp").unwrap();
    let branch_count = f
        .blocks
        .iter()
        .filter(|b| matches!(b.terminator, Terminator::Branch { .. }))
        .count();
    assert!(
        branch_count >= 2,
        "expected at least 2 Branch terminators for nested if"
    );
}

// ---------------------------------------------------------------------------
// Nested record field access
// ---------------------------------------------------------------------------

#[test]
fn lower_nested_field_access() {
    let m = lower_src(r#"let get = (r) => r.inner.value"#);
    let f = m.function("get").unwrap();
    let record_gets = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter(|(_, inst)| matches!(inst, Instruction::RecordGet { .. }))
        .count();
    assert!(
        record_gets >= 2,
        "expected at least 2 RecordGet instructions"
    );
}

// ---------------------------------------------------------------------------
// Chained function calls
// ---------------------------------------------------------------------------

#[test]
fn lower_chained_calls() {
    let m = lower_src("let id = (x) => x\nlet apply = (f, x) => f(x)");
    assert!(m.function("id").is_some());
    assert!(m.function("apply").is_some());
}

// ---------------------------------------------------------------------------
// Curried functions
// ---------------------------------------------------------------------------

#[test]
fn lower_curried_function() {
    let m = lower_src("let add = (a) => (b) => a + b");
    let f = m.function("add").unwrap();
    // Return type should be a function type (curried)
    assert!(
        matches!(f.return_type, IrType::Func(_) | IrType::Closure(_)),
        "curried function should return Func/Closure, got {:?}",
        f.return_type
    );
}

// ---------------------------------------------------------------------------
// Let-polymorphism through pipeline
// ---------------------------------------------------------------------------

#[test]
fn lower_polymorphic_identity() {
    let m = lower_src("let id = (x) => x");
    let f = m.function("id").unwrap();
    // id is ∀a. a → a — at IR level this becomes a function with one param
    assert_eq!(f.params.len(), 1);
    let entry = &f.blocks[0];
    assert!(matches!(entry.terminator, Terminator::Return(_)));
}

// ---------------------------------------------------------------------------
// Match with binding pattern
// ---------------------------------------------------------------------------

#[test]
fn lower_match_binding_pattern() {
    let m = lower_src("let unwrap = (opt) => match opt { x => x }");
    let f = m.function("unwrap").unwrap();
    // Single binding arm on a primitive type: no Switch needed.
    // We expect a Jump from the entry to the arm (or merge) block.
    let has_jump = f
        .blocks
        .iter()
        .any(|b| matches!(b.terminator, Terminator::Jump { .. }));
    assert!(has_jump, "single-arm match should emit Jump, not Switch");
}

// ---------------------------------------------------------------------------
// Empty array
// ---------------------------------------------------------------------------

#[test]
fn lower_empty_array() {
    let m = lower_src("let empty = []");
    let f = m.function("empty").unwrap();
    assert!(matches!(f.return_type, IrType::Array(_)));
}

// ---------------------------------------------------------------------------
// BUG C1/C2: Match on u64 must not fall back to ConstI32 for constants
// ---------------------------------------------------------------------------

#[test]
fn fix_match_subject_u64_const_type() {
    // Lower a match on u64. The subject type must be U64, and the
    // literal arm constant must be ConstU64, not ConstI32 (the old
    // fallback when type_map held an unresolved Var).
    let m = lower_src("let f = (n: u64) => match n { 0u64 => 1i64, _ => 0i64 }");
    let f = m.function("f").unwrap();
    // At least one block must contain ConstU64(0) for the pattern.
    let has_u64 = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, inst)| matches!(inst, Instruction::ConstU64(0)));
    assert!(
        has_u64,
        "match on u64 must use ConstU64(0) for the 0u64 pattern, not ConstI32(0)"
    );
    // The 0u64 pattern must NOT emit ConstI32(0).
    let has_i32 = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, inst)| matches!(inst, Instruction::ConstI32(0)));
    assert!(
        !has_i32,
        "match on u64 must NOT emit ConstI32(0) as the subject comparator"
    );
}

#[test]
fn fix_match_subject_usize_const_type() {
    let m = lower_src("let f = (n: usize) => match n { 0usize => 1i64, _ => 0i64 }");
    let f = m.function("f").unwrap();
    let has_usize = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, inst)| matches!(inst, Instruction::ConstUsize(0)));
    assert!(
        has_usize,
        "match on usize must use ConstUsize(0), not ConstI32(0)"
    );
}

// ---------------------------------------------------------------------------
// BUG C4: Suffixed literal patterns must produce correct discriminant values
// ---------------------------------------------------------------------------

#[test]
fn fix_match_suffixed_discriminant_u64() {
    let m = lower_src("let f = (n: u64) => match n { 42u64 => 1i64, _ => 0i64 }");
    let f = m.function("f").unwrap();
    let has_42 = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, inst)| matches!(inst, Instruction::ConstU64(42)));
    assert!(has_42, "pattern 42u64 must produce ConstU64(42), not 0");
}

#[test]
fn fix_match_suffixed_discriminant_u8() {
    let m = lower_src("let f = (n: u8) => match n { 255u8 => 1i64, _ => 0i64 }");
    let f = m.function("f").unwrap();
    let has_255 = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, inst)| matches!(inst, Instruction::ConstU8(255)));
    assert!(has_255, "pattern 255u8 must produce ConstU8(255)");
}

#[test]
fn fix_match_suffixed_discriminant_i8() {
    let m = lower_src("let f = (n: i8) => match n { 127i8 => 1i64, _ => 0i64 }");
    let f = m.function("f").unwrap();
    let has_127 = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, inst)| matches!(inst, Instruction::ConstI8(127)));
    assert!(has_127, "pattern 127i8 must produce ConstI8(127)");
}

// ---------------------------------------------------------------------------
// BUG C8: F32/F64 match arms must use correct const type, not ConstI64
// ---------------------------------------------------------------------------

#[test]
fn fix_match_f64_literal() {
    let m = lower_src("let f = (x: f64) => if x > 0.0 { 1i64 } else { 0i64 }");
    let f = m.function("f").unwrap();
    let has_f64 = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, inst)| matches!(inst, Instruction::ConstF64(v) if (v - 0.0).abs() < 1e-10));
    assert!(has_f64, "if on f64 must use ConstF64(0.0), not ConstI64(0)");
}

// ---------------------------------------------------------------------------
// BUG C9: Hex/octal/binary literals must not silently become 0
// ---------------------------------------------------------------------------

#[test]
fn fix_hex_literal_u8() {
    let m = lower_src("let x = 0xFFu8");
    let f = m.function("x").unwrap();
    assert_eq!(f.return_type, IrType::U8);
    let has_255 = f.blocks[0]
        .instructions
        .iter()
        .any(|(_, inst)| matches!(inst, Instruction::ConstU8(255)));
    assert!(has_255, "hex literal 0xFFu8 must produce ConstU8(255)");
}

#[test]
fn fix_hex_literal_u32() {
    let m = lower_src("let x = 0xDEADu32");
    let f = m.function("x").unwrap();
    assert_eq!(f.return_type, IrType::U32);
    let has_dead = f.blocks[0]
        .instructions
        .iter()
        .any(|(_, inst)| matches!(inst, Instruction::ConstU32(0xDEAD)));
    assert!(
        has_dead,
        "hex literal 0xDEADu32 must produce ConstU32(0xDEAD)"
    );
}

// ---------------------------------------------------------------------------
// BUG C1/C2: Type-map staleness — un-annotated match subject resolved through
// pattern matching must not fall back to I32.
// ---------------------------------------------------------------------------

#[test]
fn fix_match_unannotated_subject_resolved_through_pattern() {
    // 'x' has no annotation. The first arm `true` unifies x with Bool,
    // but the type_map entry for 'x' was recorded BEFORE unification
    // with arm patterns. The lowerer must still produce Bool constants,
    // not I32 fallback.
    let m = lower_src("let f = (x) => match x { true => 1i64, false => 0i64 }");
    let f = m.function("f").unwrap();
    let has_const_bool = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, inst)| matches!(inst, Instruction::ConstBool(true)));
    assert!(
        has_const_bool,
        "match on un-annotated bool subject should emit ConstBool(true), not ConstI32(1)"
    );
    let has_const_i32 = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, inst)| matches!(inst, Instruction::ConstI32(1)));
    assert!(
        !has_const_i32,
        "match on un-annotated bool subject must NOT emit ConstI32(1)"
    );
}

// ---------------------------------------------------------------------------
// BUG C11: field_index_of must work with compound (non-Ident) expressions
// ---------------------------------------------------------------------------

#[test]
fn fix_field_index_compound_expression() {
    // Calling a function that returns a record, then accessing a field.
    // The lowerer must find the field index from the type_map or inferred
    // return type, not just from simple Ident resolution.
    let m = lower_src(
        "let make_person = () => { name: \"Alice\", age: 30 }\n\
         let get_age = () => make_person().age\n",
    );
    let f = m.function("get_age").unwrap();
    let record_gets = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter(|(_, inst)| matches!(inst, Instruction::RecordGet { field, .. } if field.as_str() == "age"))
        .count();
    assert!(
        record_gets >= 1,
        "field access on compound expression (make_person().age) must produce RecordGet with field=age"
    );
}

// ---------------------------------------------------------------------------
// BUG C10: Float/Str literal patterns must not silently get discriminant 0
// ---------------------------------------------------------------------------

// Str pattern matching is not supported; can't easily test lower failure here.
