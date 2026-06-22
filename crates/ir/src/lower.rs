use std::collections::{HashMap, HashSet};

use ast::SmolStr;
use ast::ast::{BinOp, Decl, Expr, Pattern, Stmt, TemplatePart, UnaryOp};
use typechecker::{MonoType, TagVariants, TypedProgram};

use crate::{
    BasicBlock, BlockId, FuncType, Instruction, IrDecl, IrFunction, IrModule, IrType,
    MakeClosureData, RecordAllocData, TagConstructData, TagType, TagVariant, Terminator, ValueId,
    infer_instruction_type,
};

// ---------------------------------------------------------------------------
// LowerError
// ---------------------------------------------------------------------------

/// Errors produced during IR lowering.
#[derive(Debug, Clone, thiserror::Error)]
pub enum LowerError {
    #[error("unbound name in IR lowering: {0}")]
    Unbound(SmolStr),
    #[error("IR lowering not implemented for this expression shape")]
    Unimplemented,
}

// ---------------------------------------------------------------------------
// MonoType → IrType
// ---------------------------------------------------------------------------

pub(crate) fn mono_to_ir(ty: &MonoType) -> IrType {
    mono_to_ir_inner(ty, None)
}

fn mono_to_ir_inner(ty: &MonoType, tag_variants: Option<&TagVariants>) -> IrType {
    match ty {
        MonoType::I8 => IrType::I8,
        MonoType::I16 => IrType::I16,
        MonoType::I32 => IrType::I32,
        MonoType::I64 => IrType::I64,
        MonoType::U8 => IrType::U8,
        MonoType::U16 => IrType::U16,
        MonoType::U32 => IrType::U32,
        MonoType::U64 => IrType::U64,
        MonoType::Usize => IrType::Usize,
        MonoType::F32 => IrType::F32,
        MonoType::F64 => IrType::F64,
        MonoType::Bool => IrType::Bool,
        MonoType::Str => IrType::Str,
        MonoType::Unit => IrType::Unit,
        MonoType::Array(inner) => IrType::Array(Box::new(mono_to_ir_inner(inner, tag_variants))),
        MonoType::Func { params, ret } => IrType::Func(FuncType {
            params: params
                .iter()
                .map(|p| mono_to_ir_inner(p, tag_variants))
                .collect(),
            ret: Box::new(mono_to_ir_inner(ret, tag_variants)),
        }),
        MonoType::Record(fields) => IrType::Record(crate::RecordType {
            name: "anon".into(),
            fields: fields
                .iter()
                .map(|(k, v)| (k.clone(), mono_to_ir_inner(v, tag_variants)))
                .collect(),
        }),
        MonoType::Effect(inner) => IrType::Effect(Box::new(mono_to_ir_inner(inner, tag_variants))),
        MonoType::Tag { name, payload } => {
            if let Some(variants) = tag_variants.and_then(|tv| tv.get(name.as_str())) {
                let mut offset = 0;
                let ir_variants: Vec<TagVariant> = variants
                    .iter()
                    .enumerate()
                    .map(|(i, (vname, vtemplate))| {
                        let count = vtemplate.len();
                        let vpayload: Vec<IrType> = payload[offset..offset + count]
                            .iter()
                            .map(|t| mono_to_ir_inner(t, tag_variants))
                            .collect();
                        offset += count;
                        TagVariant {
                            name: vname.clone(),
                            discriminant: i as u32,
                            payload: vpayload,
                        }
                    })
                    .collect();
                IrType::Tag(TagType {
                    name: name.clone(),
                    variants: ir_variants,
                })
            } else {
                IrType::Tag(TagType {
                    name: name.clone(),
                    variants: vec![TagVariant {
                        name: name.clone(),
                        discriminant: 0,
                        payload: payload
                            .iter()
                            .map(|t| mono_to_ir_inner(t, tag_variants))
                            .collect(),
                    }],
                })
            }
        }
        MonoType::Var(_) => IrType::I32,
    }
}

/// Looks up the `IrType` for an expression span in the type map.
fn expr_ir_type(
    span: ast::span::Span,
    type_map: &HashMap<ast::span::Span, MonoType>,
    tag_variants: Option<&TagVariants>,
) -> IrType {
    type_map
        .get(&span)
        .map(|m| mono_to_ir_inner(m, tag_variants))
        .unwrap_or(IrType::I32)
}

// ---------------------------------------------------------------------------
// Free-variable analysis
// ---------------------------------------------------------------------------

fn free_vars<'a>(expr: &Expr<'a>, bound: &HashSet<&'a str>, out: &mut HashSet<SmolStr>) {
    match expr {
        Expr::Ident(name, _) => {
            if !bound.contains(name) {
                out.insert((*name).into());
            }
        }
        Expr::Lambda { params, body, .. } => {
            let mut inner = bound.clone();
            params.iter().for_each(|p| {
                inner.insert(p.name);
            });
            free_vars(body, &inner, out);
        }
        Expr::Application { func, args, .. } => {
            free_vars(func, bound, out);
            args.iter().for_each(|a| free_vars(a, bound, out));
        }
        Expr::Binary { left, right, .. } => {
            free_vars(left, bound, out);
            free_vars(right, bound, out);
        }
        Expr::Unary { operand, .. } => free_vars(operand, bound, out),
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            free_vars(condition, bound, out);
            free_vars(then_branch, bound, out);
            free_vars(else_branch, bound, out);
        }
        Expr::Block { stmts, result, .. } => {
            let mut inner = bound.clone();
            for stmt in stmts {
                match stmt {
                    Stmt::Let { pattern, value } => {
                        free_vars(value, &inner, out);
                        collect_bound_names(pattern, &mut inner);
                    }
                    Stmt::Expr(e) => free_vars(e, &inner, out),
                }
            }
            free_vars(result, &inner, out);
        }
        Expr::Match { subject, arms, .. } => {
            free_vars(subject, bound, out);
            for arm in arms {
                let mut inner = bound.clone();
                collect_bound_names(arm.pattern, &mut inner);
                free_vars(arm.body, &inner, out);
            }
        }
        Expr::Array { elems, .. } => elems.iter().for_each(|e| free_vars(e, bound, out)),
        Expr::Tuple { elems, .. } => elems.iter().for_each(|e| free_vars(e, bound, out)),
        Expr::Record { fields, .. } => fields.iter().for_each(|f| free_vars(f.value, bound, out)),
        Expr::FieldAccess { object, .. } => free_vars(object, bound, out),
        Expr::Index { array, index, .. } => {
            free_vars(array, bound, out);
            free_vars(index, bound, out);
        }
        Expr::Template { parts, .. } => {
            for part in parts {
                if let TemplatePart::Expr(e) = part {
                    free_vars(e, bound, out);
                }
            }
        }
        _ => {}
    }
}

fn collect_bound_names<'a>(pat: &Pattern<'a>, out: &mut HashSet<&'a str>) {
    match pat {
        Pattern::Binding(name, _) => {
            out.insert(name);
        }
        Pattern::Constructor { fields, .. } => {
            fields.iter().for_each(|p| collect_bound_names(p, out))
        }
        Pattern::Tuple { patterns, .. } => {
            patterns.iter().for_each(|p| collect_bound_names(p, out))
        }
        Pattern::Record { fields, .. } => {
            for f in fields {
                if let Some(p) = f.pattern {
                    collect_bound_names(p, out);
                } else {
                    out.insert(f.name);
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// FunctionBuilder
// ---------------------------------------------------------------------------

/// Builds a single `IrFunction`, threading the type map for accurate IrTypes.
struct FunctionBuilder<'a> {
    func: IrFunction,
    current_block: usize,
    locals: HashMap<SmolStr, ValueId>,
    value_types: HashMap<ValueId, IrType>,
    globals: &'a HashSet<SmolStr>,
    type_map: &'a HashMap<ast::span::Span, MonoType>,
    tag_variants: &'a TagVariants,
}

impl<'a> FunctionBuilder<'a> {
    fn new(
        name: SmolStr,
        ret: IrType,
        globals: &'a HashSet<SmolStr>,
        type_map: &'a HashMap<ast::span::Span, MonoType>,
        tag_variants: &'a TagVariants,
    ) -> Self {
        let mut func = IrFunction::new(name, ret);
        let entry_id = func.alloc_block();
        func.blocks.push(BasicBlock::new(entry_id));
        Self {
            func,
            current_block: 0,
            locals: HashMap::new(),
            value_types: HashMap::new(),
            globals,
            type_map,
            tag_variants,
        }
    }

    fn alloc_value(&mut self) -> ValueId {
        self.func.alloc_value()
    }

    fn alloc_block(&mut self) -> BlockId {
        let id = self.func.alloc_block();
        self.func.blocks.push(BasicBlock::new(id));
        id
    }

    fn set_current(&mut self, id: BlockId) {
        self.current_block = self
            .func
            .blocks
            .iter()
            .position(|b| b.id == id)
            .expect("block not found");
    }

    fn emit(&mut self, inst: Instruction) -> ValueId {
        let v = self.alloc_value();
        // Infer type BEFORE moving `inst` into the block.
        let ty = infer_instruction_type(&inst, &self.value_types, &HashMap::new());
        self.func.blocks[self.current_block]
            .instructions
            .push((Some(v), inst));
        if let Some(ty) = ty {
            self.value_types.insert(v, ty);
        }
        v
    }

    fn set_terminator(&mut self, term: Terminator) {
        self.func.blocks[self.current_block].terminator = term;
    }

    fn bind(&mut self, name: SmolStr, val: ValueId) {
        self.locals.insert(name, val);
    }
    fn lookup(&self, name: &str) -> Option<ValueId> {
        self.locals.get(name).copied()
    }

    /// Returns the `IrType` for an expression span using the type map.
    fn expr_type(&self, span: ast::span::Span) -> IrType {
        expr_ir_type(span, self.type_map, Some(self.tag_variants))
    }
}

// ---------------------------------------------------------------------------
// Expression lowering
// ---------------------------------------------------------------------------

fn lower_expr<'src>(
    fb: &mut FunctionBuilder<'_>,
    expr: &Expr<'src>,
    hoisted: &mut Vec<IrFunction>,
) -> Result<ValueId, LowerError> {
    match expr {
        Expr::IntLiteral(text, _) => Ok(fb.emit(parse_int_literal(text))),
        Expr::FloatLiteral(text, _) => Ok(fb.emit(parse_float_literal(text))),
        Expr::Bool(b, _) => Ok(fb.emit(Instruction::ConstBool(*b))),
        Expr::Str(s, _) => Ok(fb.emit(Instruction::ConstStr((*s).into()))),

        Expr::Ident(name, _) => {
            if let Some(v) = fb.lookup(name) {
                Ok(v)
            } else if fb.globals.contains(*name) {
                Ok(fb.emit(Instruction::MakeClosure(Box::new(MakeClosureData {
                    func_name: (*name).into(),
                    captures: vec![],
                }))))
            } else {
                Err(LowerError::Unbound((*name).into()))
            }
        }

        Expr::Binary {
            op, left, right, ..
        } => {
            let lv = lower_expr(fb, left, hoisted)?;
            let rv = lower_expr(fb, right, hoisted)?;
            let inst = match op {
                BinOp::Add => Instruction::Add(lv, rv),
                BinOp::Sub => Instruction::Sub(lv, rv),
                BinOp::Mul => Instruction::Mul(lv, rv),
                BinOp::Div => Instruction::Div(lv, rv),
                BinOp::Mod => Instruction::Rem(lv, rv),
                BinOp::Eq => Instruction::Eq(lv, rv),
                BinOp::Ne => Instruction::Ne(lv, rv),
                BinOp::Lt => Instruction::Lt(lv, rv),
                BinOp::Le => Instruction::Le(lv, rv),
                BinOp::Gt => Instruction::Gt(lv, rv),
                BinOp::Ge => Instruction::Ge(lv, rv),
                BinOp::And => Instruction::And(lv, rv),
                BinOp::Or => Instruction::Or(lv, rv),
            };
            Ok(fb.emit(inst))
        }

        Expr::Unary { op, operand, .. } => {
            let v = lower_expr(fb, operand, hoisted)?;
            Ok(fb.emit(match op {
                UnaryOp::Neg => Instruction::Neg(v),
                UnaryOp::Not => Instruction::Not(v),
            }))
        }

        Expr::If {
            condition,
            then_branch,
            else_branch,
            span,
        } => {
            let cond_v = lower_expr(fb, condition, hoisted)?;
            let then_id = fb.alloc_block();
            let else_id = fb.alloc_block();
            let merge_id = fb.alloc_block();

            fb.set_terminator(Terminator::Branch {
                condition: cond_v,
                then_block: then_id,
                then_args: vec![],
                else_block: else_id,
                else_args: vec![],
            });

            fb.set_current(then_id);
            let then_v = lower_expr(fb, then_branch, hoisted)?;
            fb.set_terminator(Terminator::Jump {
                target: merge_id,
                args: vec![then_v],
            });

            fb.set_current(else_id);
            let else_v = lower_expr(fb, else_branch, hoisted)?;
            fb.set_terminator(Terminator::Jump {
                target: merge_id,
                args: vec![else_v],
            });

            // Use the if-expression's inferred type for the merge block param.
            let result_ty = fb.expr_type(*span);
            fb.set_current(merge_id);
            let result_v = fb.alloc_value();
            let merge_idx = fb
                .func
                .blocks
                .iter()
                .position(|b| b.id == merge_id)
                .expect("merge block");
            fb.func.blocks[merge_idx].params.push((result_v, result_ty));
            Ok(result_v)
        }

        Expr::Block { stmts, result, .. } => {
            let saved = fb.locals.clone();
            for stmt in stmts {
                lower_stmt(fb, stmt, hoisted)?;
            }
            let v = lower_expr(fb, result, hoisted)?;
            fb.locals = saved;
            Ok(v)
        }

        Expr::Match {
            subject,
            arms,
            span,
        } => {
            let subj_v = lower_expr(fb, subject, hoisted)?;
            let subj_ty = fb.expr_type(subject.span());
            let merge_id = fb.alloc_block();
            let result_ty = fb.expr_type(*span);
            let result_v = fb.alloc_value();
            {
                let merge_idx = fb
                    .func
                    .blocks
                    .iter()
                    .position(|b| b.id == merge_id)
                    .expect("merge block");
                fb.func.blocks[merge_idx].params.push((result_v, result_ty));
            }

            let is_tag = matches!(subj_ty, IrType::Tag(_));
            let mut switch_arms: Vec<(u32, BlockId, Vec<ValueId>)> = Vec::new();
            let mut literal_arms: Vec<(i64, BlockId)> = Vec::new();
            let mut default_arm: Option<(BlockId, Vec<ValueId>)> = None;

            for arm in arms.iter() {
                let arm_id = fb.alloc_block();
                let saved = fb.locals.clone();
                fb.set_current(arm_id);
                lower_pattern(fb, arm.pattern, subj_v)?;
                let arm_v = lower_expr(fb, arm.body, hoisted)?;
                fb.set_terminator(Terminator::Jump {
                    target: merge_id,
                    args: vec![arm_v],
                });
                fb.locals = saved;

                match arm.pattern {
                    Pattern::Wildcard(_) | Pattern::Binding(_, _) => {
                        default_arm = Some((arm_id, vec![]));
                    }
                    Pattern::Literal(lit, _) => {
                        let disc = literal_discriminant(lit);
                        literal_arms.push((disc, arm_id));
                    }
                    Pattern::Constructor { name, .. } if is_tag => {
                        let disc = subj_tag_discriminant(fb, subj_v, name);
                        switch_arms.push((disc, arm_id, vec![]));
                    }
                    _ => {
                        if is_tag {
                            switch_arms.push((switch_arms.len() as u32, arm_id, vec![]));
                        } else {
                            default_arm = Some((arm_id, vec![]));
                        }
                    }
                }
            }

            if is_tag {
                // Tag types: TagDiscriminant + Switch (original path).
                let disc_v = fb.emit(Instruction::TagDiscriminant(subj_v));
                let subject_block_idx = fb
                    .func
                    .blocks
                    .iter()
                    .position(|b| b.instructions.iter().any(|(vid, _)| *vid == Some(disc_v)))
                    .unwrap_or(0);
                fb.current_block = subject_block_idx;
                fb.set_terminator(Terminator::Switch {
                    discriminant: disc_v,
                    arms: switch_arms,
                    default: default_arm,
                });
            } else {
                // Primitive types: cascading Branch chain using Eq comparisons.
                // The subject value IS the discriminant.
                // Emit arms in reverse so the first literal becomes the outermost check.
                let mut cascade_target: Option<BlockId> = default_arm.map(|(b, _)| b);
                for (disc, block_id) in literal_arms.into_iter().rev() {
                    let check_block = fb.alloc_block();
                    fb.set_current(check_block);
                    let lit_v = fb.emit(match subj_ty {
                        IrType::I8 => Instruction::ConstI8(disc as i8),
                        IrType::I16 => Instruction::ConstI16(disc as i16),
                        IrType::I32 => Instruction::ConstI32(disc as i32),
                        IrType::I64 => Instruction::ConstI64(disc),
                        IrType::U8 => Instruction::ConstU8(disc as u8),
                        IrType::U16 => Instruction::ConstU16(disc as u16),
                        IrType::U32 => Instruction::ConstU32(disc as u32),
                        IrType::U64 => Instruction::ConstU64(disc as u64),
                        IrType::Usize => Instruction::ConstUsize(disc as usize),
                        IrType::Bool => Instruction::ConstBool(disc != 0),
                        _ => Instruction::ConstI64(disc),
                    });
                    let eq_v = fb.emit(Instruction::Eq(subj_v, lit_v));
                    let else_target = cascade_target.unwrap_or_else(|| {
                        let trap = fb.alloc_block();
                        fb.set_current(trap);
                        fb.set_terminator(Terminator::Unreachable);
                        trap
                    });
                    fb.set_terminator(Terminator::Branch {
                        condition: eq_v,
                        then_block: block_id,
                        then_args: vec![],
                        else_block: else_target,
                        else_args: vec![],
                    });
                    cascade_target = Some(check_block);
                }
                // Jump from the subject block to the first check.
                let entry = cascade_target.unwrap_or_else(|| {
                    let trap = fb.alloc_block();
                    fb.set_current(trap);
                    fb.set_terminator(Terminator::Unreachable);
                    trap
                });
                let subject_block_idx = fb
                    .func
                    .blocks
                    .iter()
                    .position(|b| b.instructions.iter().any(|(vid, _)| *vid == Some(subj_v)))
                    .unwrap_or(0);
                fb.current_block = subject_block_idx;
                fb.set_terminator(Terminator::Jump {
                    target: entry,
                    args: vec![],
                });
            }

            fb.set_current(merge_id);
            Ok(result_v)
        }

        Expr::Array { elems, span } => {
            let elem_vals: Vec<ValueId> = elems
                .iter()
                .map(|e| lower_expr(fb, e, hoisted))
                .collect::<Result<_, _>>()?;
            let return_type = fb
                .type_map
                .get(span)
                .map(mono_to_ir)
                .unwrap_or(IrType::Unit);
            Ok(
                fb.emit(Instruction::CallNamed(Box::new(crate::CallNamedData {
                    name: "array_literal".into(),
                    args: elem_vals,
                    return_type,
                }))),
            )
        }

        Expr::Tuple { elems, .. } => {
            let elem_vals: Vec<ValueId> = elems
                .iter()
                .map(|e| lower_expr(fb, e, hoisted))
                .collect::<Result<_, _>>()?;
            Ok(
                fb.emit(Instruction::TagConstruct(Box::new(TagConstructData {
                    type_name: "Tuple".into(),
                    variant: "Tuple".into(),
                    discriminant: 0,
                    payload: elem_vals,
                }))),
            )
        }

        Expr::Record { fields, .. } => {
            let field_vals: Vec<ValueId> = fields
                .iter()
                .map(|f| lower_expr(fb, f.value, hoisted))
                .collect::<Result<_, _>>()?;
            Ok(fb.emit(Instruction::RecordAlloc(Box::new(RecordAllocData {
                type_name: "anon".into(),
                fields: field_vals,
            }))))
        }

        Expr::FieldAccess { object, field, .. } => {
            let obj_v = lower_expr(fb, object, hoisted)?;
            // Compute the field index from the object's inferred record type.
            let field_index = field_index_of(fb, object, field);
            Ok(fb.emit(Instruction::RecordGet {
                record: obj_v,
                field: (*field).into(),
                field_index,
            }))
        }

        Expr::Index { array, index, .. } => {
            let arr_v = lower_expr(fb, array, hoisted)?;
            let idx_v = lower_expr(fb, index, hoisted)?;
            Ok(fb.emit(Instruction::ArrayGet {
                array: arr_v,
                index: idx_v,
            }))
        }

        Expr::Template { parts, .. } => {
            let mut part_vals = Vec::new();
            for part in parts {
                match part {
                    TemplatePart::Str(s) => {
                        part_vals.push(fb.emit(Instruction::ConstStr((*s).into())));
                    }
                    TemplatePart::Expr(e) => {
                        part_vals.push(lower_expr(fb, e, hoisted)?);
                    }
                }
            }
            Ok(fb.emit(Instruction::StrConcat { parts: part_vals }))
        }

        Expr::Lambda { params, body, .. } => {
            let param_names: HashSet<&str> = params.iter().map(|p| p.name).collect();
            let mut frees = HashSet::new();
            free_vars(body, &param_names, &mut frees);
            let captures: Vec<SmolStr> = frees
                .into_iter()
                .filter(|n| fb.locals.contains_key(n.as_str()) || fb.globals.contains(n))
                .collect();

            let inner_name: SmolStr = format!("__lambda_{}", fb.func.next_value_id).into();

            // Determine the body return type from the type map.
            let body_ret_ty = fb.expr_type(body.span());

            let mut inner_fb = FunctionBuilder::new(
                inner_name.clone(),
                body_ret_ty,
                fb.globals,
                fb.type_map,
                fb.tag_variants,
            );

            // Capture params — type from outer scope value_types.
            for cap in &captures {
                let v = inner_fb.alloc_value();
                let cap_ty = fb
                    .lookup(cap.as_str())
                    .and_then(|cv| fb.value_types.get(&cv).cloned())
                    .unwrap_or(IrType::I32);
                inner_fb.func.params.push((v, cap.clone(), cap_ty));
                inner_fb.bind(cap.clone(), v);
            }
            // Declared params — resolve type from annotation or type_map.
            for p in params.iter() {
                let v = inner_fb.alloc_value();
                let param_ty =
                    p.ty.and_then(|ann| typechecker::infer::type_expr_to_mono(ann).ok())
                        .as_ref()
                        .map(mono_to_ir)
                        .unwrap_or(IrType::I32);
                inner_fb.func.params.push((v, p.name.into(), param_ty));
                inner_fb.bind(p.name.into(), v);
            }

            let body_v = lower_expr(&mut inner_fb, body, hoisted)?;
            inner_fb.set_terminator(Terminator::Return(body_v));
            hoisted.push(inner_fb.func);

            let capture_vals: Vec<ValueId> = captures
                .iter()
                .filter_map(|n| fb.lookup(n.as_str()))
                .collect();
            Ok(fb.emit(Instruction::MakeClosure(Box::new(MakeClosureData {
                func_name: inner_name,
                captures: capture_vals,
            }))))
        }

        Expr::Application { func, args, span } => {
            let arg_vals: Vec<ValueId> = args
                .iter()
                .map(|a| lower_expr(fb, a, hoisted))
                .collect::<Result<_, _>>()?;
            let return_type = fb
                .type_map
                .get(span)
                .map(mono_to_ir)
                .unwrap_or(IrType::Unit);
            match func {
                Expr::Ident(name, _) => Ok(fb.emit(Instruction::CallNamed(Box::new(
                    crate::CallNamedData {
                        name: (*name).into(),
                        args: arg_vals,
                        return_type,
                    },
                )))),
                _ => {
                    let callee = lower_expr(fb, func, hoisted)?;
                    Ok(fb.emit(Instruction::CallIndirect(Box::new(
                        crate::CallIndirectData {
                            callee,
                            args: arg_vals,
                            return_type,
                        },
                    ))))
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers for discriminant and field-index resolution
// ---------------------------------------------------------------------------

/// Returns the discriminant value for a literal pattern arm.
fn literal_discriminant(lit: &ast::ast::LiteralPattern<'_>) -> i64 {
    match lit {
        ast::ast::LiteralPattern::Bool(true) => 1,
        ast::ast::LiteralPattern::Bool(false) => 0,
        ast::ast::LiteralPattern::Int(s) => s.parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

/// Returns the variant discriminant for a constructor pattern by looking up
/// the subject value's `IrType::Tag` in `value_types`.
fn subj_tag_discriminant(fb: &FunctionBuilder<'_>, subj_v: ValueId, variant_name: &str) -> u32 {
    if let Some(IrType::Tag(tag)) = fb.value_types.get(&subj_v)
        && let Some(v) = tag.variants.iter().find(|v| v.name == variant_name)
    {
        return v.discriminant;
    }
    0
}

/// Returns the field index of `field` in the record type of `object`.
fn field_index_of(fb: &FunctionBuilder<'_>, object: &Expr<'_>, field: &str) -> u32 {
    let span = object.span();
    if let Some(MonoType::Record(fields)) = fb.type_map.get(&span) {
        // BTreeMap keys are sorted — index is the sorted position.
        return fields.keys().position(|k| k.as_str() == field).unwrap_or(0) as u32;
    }
    0
}

// ---------------------------------------------------------------------------
// Statement and pattern lowering
// ---------------------------------------------------------------------------

fn lower_stmt<'src>(
    fb: &mut FunctionBuilder<'_>,
    stmt: &Stmt<'src>,
    hoisted: &mut Vec<IrFunction>,
) -> Result<(), LowerError> {
    match stmt {
        Stmt::Let { pattern, value } => {
            let v = lower_expr(fb, value, hoisted)?;
            bind_pattern_local(fb, pattern, v);
        }
        Stmt::Expr(e) => {
            lower_expr(fb, e, hoisted)?;
        }
    }
    Ok(())
}

fn lower_pattern<'src>(
    fb: &mut FunctionBuilder<'_>,
    pat: &Pattern<'src>,
    scrutinee: ValueId,
) -> Result<(), LowerError> {
    match pat {
        Pattern::Binding(name, _) => fb.bind((*name).into(), scrutinee),
        Pattern::Wildcard(_) | Pattern::Literal(_, _) => {}
        Pattern::Constructor { fields, .. } => {
            for (i, p) in fields.iter().enumerate() {
                let fv = fb.emit(Instruction::TagGet {
                    value: scrutinee,
                    index: i as u32,
                });
                lower_pattern(fb, p, fv)?;
            }
        }
        Pattern::Tuple { patterns, .. } => {
            for (i, p) in patterns.iter().enumerate() {
                let fv = fb.emit(Instruction::TagGet {
                    value: scrutinee,
                    index: i as u32,
                });
                lower_pattern(fb, p, fv)?;
            }
        }
        Pattern::Record { fields, .. } => {
            for (i, f) in fields.iter().enumerate() {
                let fv = fb.emit(Instruction::RecordGet {
                    record: scrutinee,
                    field: f.name.into(),
                    field_index: i as u32,
                });
                if let Some(p) = f.pattern {
                    lower_pattern(fb, p, fv)?;
                } else {
                    fb.bind(f.name.into(), fv);
                }
            }
        }
    }
    Ok(())
}

fn bind_pattern_local<'src>(fb: &mut FunctionBuilder<'_>, pat: &Pattern<'src>, v: ValueId) {
    match pat {
        Pattern::Binding(name, _) => fb.bind((*name).into(), v),
        Pattern::Wildcard(_) => {}
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Literal parsing
// ---------------------------------------------------------------------------

fn parse_int_literal(text: &str) -> Instruction {
    if let Some(s) = text.strip_suffix("i64") {
        return Instruction::ConstI64(s.parse().unwrap_or(0));
    }
    if let Some(s) = text.strip_suffix("i32") {
        return Instruction::ConstI32(s.parse().unwrap_or(0));
    }
    if let Some(s) = text.strip_suffix("i16") {
        return Instruction::ConstI16(s.parse().unwrap_or(0));
    }
    if let Some(s) = text.strip_suffix("i8") {
        return Instruction::ConstI8(s.parse().unwrap_or(0));
    }
    if let Some(s) = text.strip_suffix("u64") {
        return Instruction::ConstU64(s.parse().unwrap_or(0));
    }
    if let Some(s) = text.strip_suffix("u32") {
        return Instruction::ConstU32(s.parse().unwrap_or(0));
    }
    if let Some(s) = text.strip_suffix("u16") {
        return Instruction::ConstU16(s.parse().unwrap_or(0));
    }
    if let Some(s) = text.strip_suffix("u8") {
        return Instruction::ConstU8(s.parse().unwrap_or(0));
    }
    if let Some(s) = text.strip_suffix("usize") {
        return Instruction::ConstUsize(s.parse().unwrap_or(0));
    }
    Instruction::ConstI32(text.parse().unwrap_or(0))
}

fn parse_float_literal(text: &str) -> Instruction {
    if let Some(s) = text.strip_suffix("f32") {
        return Instruction::ConstF32(s.parse().unwrap_or(0.0));
    }
    if let Some(s) = text.strip_suffix("f64") {
        return Instruction::ConstF64(s.parse().unwrap_or(0.0));
    }
    Instruction::ConstF64(text.parse().unwrap_or(0.0))
}

// ---------------------------------------------------------------------------
// Top-level lowering
// ---------------------------------------------------------------------------

/// Derives the function's return type from the type map.
fn decl_return_type(
    decl_span: ast::span::Span,
    value: &Expr<'_>,
    type_map: &HashMap<ast::span::Span, MonoType>,
) -> IrType {
    let Some(mono) = type_map.get(&decl_span) else {
        return IrType::I32;
    };
    match (value, mono) {
        (Expr::Lambda { .. }, MonoType::Func { ret, .. }) => mono_to_ir(ret),
        (_, other) => mono_to_ir(other),
    }
}

fn lower_decl<'src>(
    decl: &Decl<'src>,
    globals: &HashSet<SmolStr>,
    type_map: &HashMap<ast::span::Span, MonoType>,
    tag_variants: &TagVariants,
    module: &mut IrModule,
) -> Result<(), LowerError> {
    match decl {
        Decl::Bind {
            name, value, span, ..
        } => {
            let ret_ty = decl_return_type(*span, value, type_map);
            let mut hoisted = Vec::new();

            match value {
                Expr::Lambda { params, body, .. } => {
                    let mut fb = FunctionBuilder::new(
                        (*name).into(),
                        ret_ty,
                        globals,
                        type_map,
                        tag_variants,
                    );
                    for p in params.iter() {
                        let v = fb.alloc_value();
                        // Resolve param type from annotation, or from the Func type in type_map.
                        let param_ty = p
                            .ty
                            .and_then(|ann| typechecker::infer::type_expr_to_mono(ann).ok())
                            .as_ref()
                            .map(mono_to_ir)
                            .or_else(|| {
                                // Extract from the function's MonoType in the map.
                                type_map.get(span).and_then(|mono| match mono {
                                    MonoType::Func { params: ptys, .. } => {
                                        let idx = params.iter().position(|q| q.name == p.name)?;
                                        Some(mono_to_ir(&ptys[idx]))
                                    }
                                    _ => None,
                                })
                            })
                            .unwrap_or(IrType::I32);
                        fb.func.params.push((v, p.name.into(), param_ty));
                        fb.bind(p.name.into(), v);
                    }
                    let body_v = lower_expr(&mut fb, body, &mut hoisted)?;
                    fb.set_terminator(Terminator::Return(body_v));
                    module.decls.push(IrDecl::Function(fb.func));
                }
                other => {
                    let mut fb = FunctionBuilder::new(
                        (*name).into(),
                        ret_ty,
                        globals,
                        type_map,
                        tag_variants,
                    );
                    let v = lower_expr(&mut fb, other, &mut hoisted)?;
                    fb.set_terminator(Terminator::Return(v));
                    module.decls.push(IrDecl::Function(fb.func));
                }
            }

            for f in hoisted {
                module.decls.push(IrDecl::Function(f));
            }
            Ok(())
        }

        Decl::Use { path, .. } => {
            module.imports.push(path.join("::").into());
            Ok(())
        }

        Decl::TypeAlias {
            name, rhs: _, span, ..
        } => {
            // Use the type_map to get the resolved canonical type.
            if let Some(mono) = type_map.get(span) {
                module.decls.push(IrDecl::TypeAlias {
                    name: (*name).into(),
                    ty: mono_to_ir(mono),
                });
            }
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Lowers a type-checked program into a flat [`IrModule`].
///
/// Every `let` binding becomes an `IrFunction`. Inner lambdas that capture
/// variables are hoisted and referenced via `MakeClosure`. `use` declarations
/// become module imports. `type` aliases are registered as `IrDecl::TypeAlias`.
///
/// # Errors
///
/// Returns [`LowerError`] if an identifier is unbound or a construct is
/// not yet lowerable.
pub fn lower(typed: &TypedProgram<'_>) -> Result<IrModule, LowerError> {
    let mut module = IrModule::new();

    let globals: HashSet<SmolStr> = typed
        .ast
        .decls
        .iter()
        .filter_map(|d| match d {
            Decl::Bind { name, .. } => Some((*name).into()),
            _ => None,
        })
        .collect();

    for decl in &typed.ast.decls {
        lower_decl(
            decl,
            &globals,
            &typed.type_map,
            &typed.tag_variants,
            &mut module,
        )?;
    }

    Ok(module)
}
