use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use ast::SmolStr;
use ast::ast::{BinOp, Decl, Expr, Pattern, Stmt, TemplatePart, UnaryOp};
use ast::ast::{LiteralPattern, NodeId};
use typechecker::{MonoType, PolyType, TagVariants, TypedProgram};

use crate::{
    BasicBlock, BlockId, FuncType, Instruction, IrDecl, IrFunction, IrModule, IrType,
    MakeClosureData, RecordAllocData, TagConstructData, TagGetData, TagType, TagVariant,
    Terminator, ValueId, infer_instruction_type,
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

fn mono_to_ir_inner(
    ty: &MonoType,
    tag_variants: Option<&TagVariants>,
    type_args: &HashMap<typechecker::TypeId, IrType>,
) -> IrType {
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
        MonoType::Array(inner) => {
            IrType::Array(Box::new(mono_to_ir_inner(inner, tag_variants, type_args)))
        }
        MonoType::Func { params, ret } => IrType::Closure(Box::new(FuncType {
            params: params
                .iter()
                .map(|p| mono_to_ir_inner(p, tag_variants, type_args))
                .collect(),
            ret: Box::new(mono_to_ir_inner(ret, tag_variants, type_args)),
        })),
        MonoType::Record(fields) => IrType::Record(crate::RecordType {
            name: "anon".into(),
            fields: fields
                .iter()
                .map(|(k, v)| (k.clone(), mono_to_ir_inner(v, tag_variants, type_args)))
                .collect(),
        }),
        MonoType::Effect(inner) => {
            IrType::Effect(Box::new(mono_to_ir_inner(inner, tag_variants, type_args)))
        }
        MonoType::Tag { name, payload } => {
            thread_local! {
                static EXPANDING: std::cell::RefCell<std::collections::HashSet<SmolStr>> =
                    std::cell::RefCell::new(std::collections::HashSet::new());
            }

            let is_expanding = EXPANDING.with(|cell| cell.borrow().contains(name));
            if is_expanding {
                return IrType::Tag(TagType {
                    name: name.clone(),
                    variants: vec![],
                });
            }

            EXPANDING.with(|cell| cell.borrow_mut().insert(name.clone()));

            let res = if let Some(variants) = tag_variants.and_then(|tv| tv.get(name.as_str())) {
                // Use the combined payload from tag_variants (which has all
                // variant payloads flattened) instead of the MonoType payload
                // (which only has the constructor's own payload).
                let combined: Vec<MonoType> = variants
                    .iter()
                    .flat_map(|(_, ptys)| ptys.iter().cloned())
                    .collect();
                // Build a substitution from the tag_variants' template type
                // variables (e.g. `Var(opt_a)` from the prelude) to the
                // concrete payload types from this MonoType::Tag. Without
                // this, prelude-internal type variables default to I32,
                // causing TagGet to infer the wrong type for payload fields.
                let mut local_type_args = type_args.clone();
                for (template, actual) in combined.iter().zip(payload.iter()) {
                    if let MonoType::Var(id) | MonoType::IntVar(id) | MonoType::FloatVar(id) =
                        template
                    {
                        let actual_ir = mono_to_ir_inner(actual, tag_variants, type_args);
                        local_type_args.entry(*id).or_insert(actual_ir);
                    }
                }
                let mut offset = 0;
                let ir_variants: Vec<TagVariant> = variants
                    .iter()
                    .enumerate()
                    .map(|(i, (vname, vtemplate))| {
                        let count = vtemplate.len();
                        let vpayload: Vec<IrType> = combined[offset..offset + count]
                            .iter()
                            .map(|t| mono_to_ir_inner(t, tag_variants, &local_type_args))
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
                            .map(|t| mono_to_ir_inner(t, tag_variants, type_args))
                            .collect(),
                    }],
                })
            };

            EXPANDING.with(|cell| cell.borrow_mut().remove(name));
            res
        }
        MonoType::Var(id) | MonoType::IntVar(id) | MonoType::FloatVar(id) => {
            if let Some(concrete) = type_args.get(id) {
                concrete.clone()
            } else {
                match ty {
                    MonoType::FloatVar(_) => IrType::F64,
                    _ => IrType::I32,
                }
            }
        }
    }
}

/// Looks up the `IrType` for an expression [`NodeId`] in the type map.
fn expr_ir_type(
    id: NodeId,
    type_map: &HashMap<NodeId, MonoType>,
    tag_variants: Option<&TagVariants>,
    type_args: &HashMap<typechecker::TypeId, IrType>,
) -> IrType {
    type_map
        .get(&id)
        .map(|m| mono_to_ir_inner(m, tag_variants, type_args))
        .unwrap_or(IrType::I32)
}

// ---------------------------------------------------------------------------
// Free-variable analysis
// ---------------------------------------------------------------------------

fn free_vars<'a>(expr: &Expr<'a>, bound: &HashSet<&'a str>, out: &mut HashSet<SmolStr>) {
    match expr {
        Expr::Ident(_, name, _) => {
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
        Pattern::Binding(_, name, _) => {
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
    value_globals: &'a HashSet<SmolStr>,
    type_map: &'a HashMap<NodeId, MonoType>,
    tag_variants: &'a TagVariants,
    type_args: HashMap<typechecker::TypeId, IrType>,
}

impl<'a> FunctionBuilder<'a> {
    fn new(
        name: SmolStr,
        ret: IrType,
        globals: &'a HashSet<SmolStr>,
        value_globals: &'a HashSet<SmolStr>,
        type_map: &'a HashMap<NodeId, MonoType>,
        tag_variants: &'a TagVariants,
        type_args: HashMap<typechecker::TypeId, IrType>,
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
            value_globals,
            type_map,
            tag_variants,
            type_args,
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
        let ty =
            infer_instruction_type(&inst, &self.value_types, &HashMap::new(), self.tag_variants);
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

    /// Returns the `IrType` for an expression by its [`NodeId`] using the type map.
    fn expr_type(&self, id: NodeId) -> IrType {
        expr_ir_type(id, self.type_map, Some(self.tag_variants), &self.type_args)
    }

    /// Converts a `MonoType` to `IrType` using this builder's tag_variants.
    fn mono_to_ir(&self, ty: &MonoType) -> IrType {
        mono_to_ir_inner(ty, Some(self.tag_variants), &self.type_args)
    }
}

// ---------------------------------------------------------------------------
// Monomorphization Helpers
// ---------------------------------------------------------------------------

struct MonoCtx<'a, 'src> {
    queue: &'a mut Vec<(
        String,
        &'src Decl<'src>,
        HashMap<typechecker::TypeId, IrType>,
    )>,
    generated: &'a mut HashSet<String>,
    global_decls: &'a HashMap<&'src str, &'src Decl<'src>>,
    env: &'a typechecker::TypeEnv,
}

fn extract_type_args(
    poly: &MonoType,
    concrete: &MonoType,
    type_args: &mut HashMap<typechecker::TypeId, IrType>,
    tag_variants: &TagVariants,
) {
    match (poly, concrete) {
        (MonoType::Var(id) | MonoType::IntVar(id) | MonoType::FloatVar(id), _) => {
            let ir_ty = mono_to_ir_inner(concrete, Some(tag_variants), type_args);
            type_args.insert(*id, ir_ty);
        }
        (MonoType::Array(inner_poly), MonoType::Array(inner_concrete)) => {
            extract_type_args(inner_poly, inner_concrete, type_args, tag_variants);
        }
        (
            MonoType::Func {
                params: p_poly,
                ret: r_poly,
            },
            MonoType::Func {
                params: p_concrete,
                ret: r_concrete,
            },
        ) => {
            for (p_p, p_c) in p_poly.iter().zip(p_concrete.iter()) {
                extract_type_args(p_p, p_c, type_args, tag_variants);
            }
            extract_type_args(r_poly, r_concrete, type_args, tag_variants);
        }
        (MonoType::Record(fields_poly), MonoType::Record(fields_concrete)) => {
            for (k, v_poly) in fields_poly.iter() {
                if let Some(v_concrete) = fields_concrete.get(k) {
                    extract_type_args(v_poly, v_concrete, type_args, tag_variants);
                }
            }
        }
        (
            MonoType::Tag {
                name: name_poly,
                payload: payload_poly,
            },
            MonoType::Tag {
                name: name_concrete,
                payload: payload_concrete,
            },
        ) => {
            if name_poly == name_concrete {
                for (p_poly, p_concrete) in payload_poly.iter().zip(payload_concrete.iter()) {
                    extract_type_args(p_poly, p_concrete, type_args, tag_variants);
                }
            }
        }
        (MonoType::Effect(inner_poly), MonoType::Effect(inner_concrete)) => {
            extract_type_args(inner_poly, inner_concrete, type_args, tag_variants);
        }
        _ => {}
    }
}

fn format_ir_type_mangled(ty: &IrType) -> String {
    let mangled = match ty {
        IrType::I8 => "I8".to_string(),
        IrType::I16 => "I16".to_string(),
        IrType::I32 => "I32".to_string(),
        IrType::I64 => "I64".to_string(),
        IrType::U8 => "U8".to_string(),
        IrType::U16 => "U16".to_string(),
        IrType::U32 => "U32".to_string(),
        IrType::U64 => "U64".to_string(),
        IrType::Usize => "Usize".to_string(),
        IrType::F32 => "F32".to_string(),
        IrType::F64 => "F64".to_string(),
        IrType::Bool => "Bool".to_string(),
        IrType::Str => "Str".to_string(),
        IrType::Unit => "Unit".to_string(),
        IrType::Array(inner) => format!("Arr{}", format_ir_type_mangled(inner)),
        IrType::Record(rt) => {
            let mut s = format!("Rec_{}", rt.name);
            for (name, f_ty) in &rt.fields {
                s.push_str(&format!("_{}_{}", name, format_ir_type_mangled(f_ty)));
            }
            s
        }
        IrType::Func(ft) => {
            let mut s = "Func".to_string();
            for p in &ft.params {
                s.push_str(&format!("_{}", format_ir_type_mangled(p)));
            }
            s.push_str(&format!("_Ret_{}", format_ir_type_mangled(&ft.ret)));
            s
        }
        IrType::Closure(ft) => {
            let mut s = "Closure".to_string();
            for p in &ft.params {
                s.push_str(&format!("_{}", format_ir_type_mangled(p)));
            }
            s.push_str(&format!("_Ret_{}", format_ir_type_mangled(&ft.ret)));
            s
        }
        IrType::Tag(tt) => {
            format!("Tag_{}", tt.name)
        }
        IrType::Effect(inner) => format!("Eff{}", format_ir_type_mangled(inner)),
    };
    mangled
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

fn mangle_name(
    base_name: &str,
    quantified: &[typechecker::TypeId],
    type_args: &HashMap<typechecker::TypeId, IrType>,
) -> String {
    if quantified.is_empty() {
        return base_name.to_string();
    }
    let mut name = base_name.to_string();
    for q in quantified {
        let ir_ty = type_args.get(q).cloned().unwrap_or(IrType::Unit);
        name.push('_');
        name.push_str(&format_ir_type_mangled(&ir_ty));
    }
    name
}

fn resolve_and_queue_global<'src>(
    name: &str,
    expr_id: NodeId,
    fb: &FunctionBuilder<'_>,
    ctx: &mut MonoCtx<'_, 'src>,
) -> String {
    if let Some(decl) = ctx.global_decls.get(name) {
        let concrete_ty = fb.type_map.get(&expr_id).cloned().unwrap_or(MonoType::Unit);
        let poly = ctx
            .env
            .lookup(name)
            .cloned()
            .unwrap_or_else(|| PolyType::mono(concrete_ty.clone()));

        let mut type_args = HashMap::new();
        type_args.extend(fb.type_args.clone());
        extract_type_args(&poly.body, &concrete_ty, &mut type_args, fb.tag_variants);

        let mangled = mangle_name(name, &poly.quantified, &type_args);
        if !ctx.generated.contains(&mangled) {
            ctx.generated.insert(mangled.clone());
            ctx.queue.push((mangled.clone(), decl, type_args));
        }
        mangled
    } else {
        name.to_string()
    }
}

/// Qualify method names for Option/Result/Effect builtins.
/// Method calls are desugared to bare names by the parser (e.g. `option.map(f)`
/// becomes `map(option, f)`), but the runtime registers them with qualified
/// names (`"Option.map"`, `"Result.map"`, `"Effect.map"`).  The lowerer uses
/// the receiver's type from the type map to emit the correct qualified name.
fn qualify_method_name<'a>(name: &'a str, receiver_type: Option<&MonoType>) -> Cow<'a, str> {
    match (name, receiver_type) {
        ("map" | "flat_map", Some(MonoType::Tag { name: type_name, .. })) => {
            let qualified = match type_name.as_str() {
                "Option" => Cow::Owned(format!("Option.{name}")),
                "Result" => Cow::Owned(format!("Result.{name}")),
                "Effect" => Cow::Owned(format!("Effect.{name}")),
                _ => return Cow::Borrowed(name),
            };
            qualified
        }
        _ => Cow::Borrowed(name),
    }
}

fn lower_expr<'src>(
    fb: &mut FunctionBuilder<'_>,
    expr: &Expr<'src>,
    hoisted: &mut Vec<IrFunction>,
    ctx: &mut MonoCtx<'_, 'src>,
) -> Result<ValueId, LowerError> {
    match expr {
        Expr::IntLiteral(_, text, _) => Ok(fb.emit(parse_int_literal(text))),
        Expr::FloatLiteral(_, text, _) => Ok(fb.emit(parse_float_literal(text))),
        Expr::Bool(_, b, _) => Ok(fb.emit(Instruction::ConstBool(*b))),
        Expr::Str(_, s, _) => Ok(fb.emit(Instruction::ConstStr((*s).into()))),

        Expr::Ident(_, name, _) => {
            if let Some(v) = fb.lookup(name) {
                if let Some(ty) = fb.value_types.get(&v)
                    && ty.is_heap_type()
                {
                    fb.emit(Instruction::Retain(v));
                }
                Ok(v)
            } else if let Some((tag_type, disc, _)) = find_tag_constructor(name, fb.tag_variants) {
                // Bare tag constructor like `None` (0-arg). Emit a
                // `TagConstruct` with no payload.
                Ok(
                    fb.emit(Instruction::TagConstruct(Box::new(TagConstructData {
                        type_name: tag_type.into(),
                        variant: (*name).into(),
                        discriminant: disc,
                        payload: vec![],
                    }))),
                )
            } else if fb.globals.contains(*name) {
                let is_func = fb
                    .type_map
                    .get(&expr.id())
                    .map(|t| matches!(t, MonoType::Func { .. }))
                    .unwrap_or(false);
                if is_func && !fb.value_globals.contains::<str>(name) {
                    let mangled = resolve_and_queue_global(name, expr.id(), fb, ctx);
                    Ok(fb.emit(Instruction::MakeClosure(Box::new(MakeClosureData {
                        func_name: mangled.into(),
                        captures: vec![],
                    }))))
                } else {
                    let return_type = fb
                        .type_map
                        .get(&expr.id())
                        .map(|m| fb.mono_to_ir(m))
                        .unwrap_or(IrType::Unit);
                    let mangled = resolve_and_queue_global(name, expr.id(), fb, ctx);
                    Ok(
                        fb.emit(Instruction::CallNamed(Box::new(crate::CallNamedData {
                            name: mangled.into(),
                            args: vec![],
                            return_type,
                        }))),
                    )
                }
            } else {
                Err(LowerError::Unbound((*name).into()))
            }
        }

        Expr::Binary {
            op, left, right, ..
        } => {
            let lv = lower_expr(fb, left, hoisted, ctx)?;
            let rv = lower_expr(fb, right, hoisted, ctx)?;
            let inst = match op {
                BinOp::Add => {
                    // String concatenation: str + str → Str.concat(lv, rv)
                    if let Some(ty) = fb.value_types.get(&lv)
                        && matches!(ty, IrType::Str)
                    {
                        let ret_ty = fb.expr_type(expr.id());
                        return Ok(fb.emit(Instruction::CallNamed(Box::new(
                            crate::CallNamedData {
                                name: "Str.concat".into(),
                                args: vec![lv, rv],
                                return_type: ret_ty,
                            },
                        ))));
                    }
                    Instruction::Add(lv, rv)
                }
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
            let v = lower_expr(fb, operand, hoisted, ctx)?;
            Ok(fb.emit(match op {
                UnaryOp::Neg => Instruction::Neg(v),
                UnaryOp::Not => Instruction::Not(v),
            }))
        }

        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let cond_v = lower_expr(fb, condition, hoisted, ctx)?;
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
            let then_v = lower_expr(fb, then_branch, hoisted, ctx)?;
            fb.set_terminator(Terminator::Jump {
                target: merge_id,
                args: vec![then_v],
            });

            fb.set_current(else_id);
            let else_v = lower_expr(fb, else_branch, hoisted, ctx)?;
            fb.set_terminator(Terminator::Jump {
                target: merge_id,
                args: vec![else_v],
            });

            // Use the actual emitted type of then_v to prevent verifier mismatches
            // in cases where generic types temporarily default to I32 in the type map.
            let final_ty = fb
                .value_types
                .get(&then_v)
                .cloned()
                .unwrap_or_else(|| fb.expr_type(expr.id()));

            fb.set_current(merge_id);
            let result_v = fb.alloc_value();
            fb.value_types.insert(result_v, final_ty.clone());
            let merge_idx = fb
                .func
                .blocks
                .iter()
                .position(|b| b.id == merge_id)
                .expect("merge block");
            fb.func.blocks[merge_idx].params.push((result_v, final_ty));
            Ok(result_v)
        }

        Expr::Block { stmts, result, .. } => {
            let saved = fb.locals.clone();
            for stmt in stmts {
                lower_stmt(fb, stmt, hoisted, ctx)?;
            }
            let v = lower_expr(fb, result, hoisted, ctx)?;
            let locals_to_release: Vec<ValueId> = fb
                .locals
                .iter()
                .filter(|(name, val)| !saved.contains_key(name.as_str()) && **val != v)
                .map(|(_, val)| *val)
                .collect();
            for val in locals_to_release {
                if let Some(ty) = fb.value_types.get(&val)
                    && ty.is_heap_type()
                {
                    fb.emit(Instruction::Release(val));
                }
            }
            fb.locals = saved;
            Ok(v)
        }

        Expr::Match { subject, arms, .. } => {
            let subj_v = lower_expr(fb, subject, hoisted, ctx)?;
            let subj_ty = fb.expr_type(subject.id());
            let merge_id = fb.alloc_block();
            let result_v = fb.alloc_value();
            let mut resolved_result_ty: Option<IrType> = None;

            let is_tag = matches!(subj_ty, IrType::Tag(_));
            let mut switch_arms: Vec<(u32, BlockId, Vec<ValueId>)> = Vec::new();
            let mut literal_arms: Vec<(&LiteralPattern<'src>, BlockId)> = Vec::new();
            let mut default_arm: Option<(BlockId, Vec<ValueId>)> = None;

            for arm in arms.iter() {
                let arm_id = fb.alloc_block();
                let saved = fb.locals.clone();
                fb.set_current(arm_id);
                lower_pattern(fb, arm.pattern, subj_v)?;
                let arm_v = lower_expr(fb, arm.body, hoisted, ctx)?;
                if resolved_result_ty.is_none() {
                    resolved_result_ty = fb.value_types.get(&arm_v).cloned();
                }
                let locals_to_release: Vec<ValueId> = fb
                    .locals
                    .iter()
                    .filter(|(name, val)| !saved.contains_key(name.as_str()) && **val != arm_v)
                    .map(|(_, val)| *val)
                    .collect();
                for val in locals_to_release {
                    if let Some(ty) = fb.value_types.get(&val)
                        && ty.is_heap_type()
                    {
                        fb.emit(Instruction::Release(val));
                    }
                }
                if subj_v != arm_v
                    && let Some(ty) = fb.value_types.get(&subj_v)
                    && ty.is_heap_type()
                {
                    fb.emit(Instruction::Release(subj_v));
                }
                fb.set_terminator(Terminator::Jump {
                    target: merge_id,
                    args: vec![arm_v],
                });
                fb.locals = saved;

                match arm.pattern {
                    Pattern::Wildcard(_, _) | Pattern::Binding(_, _, _) => {
                        default_arm = Some((arm_id, vec![]));
                    }
                    Pattern::Literal(_, lit, _) => {
                        literal_arms.push((lit, arm_id));
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

            // Lock in the merge block parameter type from the first arm's
            // actual emitted type to prevent I32/I64 Cranelift verifier
            // mismatches when the AST type map contains an unresolved generic.
            let final_ty = resolved_result_ty.unwrap_or_else(|| fb.expr_type(expr.id()));
            fb.value_types.insert(result_v, final_ty.clone());
            {
                let merge_idx = fb
                    .func
                    .blocks
                    .iter()
                    .position(|b| b.id == merge_id)
                    .expect("merge block");
                fb.func.blocks[merge_idx].params.push((result_v, final_ty));
            }

            if is_tag {
                // Tag types: TagDiscriminant + Switch.
                // Find the block containing subj_v (the entry block) BEFORE
                // switching current_block, so TagDiscriminant and Switch are
                // emitted into the correct block (not the last arm's block).
                let subject_block_idx = fb
                    .func
                    .blocks
                    .iter()
                    .position(|b| {
                        b.instructions.iter().any(|(vid, _)| *vid == Some(subj_v))
                            || b.params.iter().any(|(vid, _)| *vid == subj_v)
                    })
                    .unwrap_or(0);
                fb.current_block = subject_block_idx;
                let disc_v = fb.emit(Instruction::TagDiscriminant(subj_v));
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
                for (lit, block_id) in literal_arms.into_iter().rev() {
                    let check_block = fb.alloc_block();
                    fb.set_current(check_block);
                    let lit_v = match lit {
                        LiteralPattern::Str(s) => fb.emit(Instruction::ConstStr((*s).into())),
                        LiteralPattern::Bool(b) => fb.emit(Instruction::ConstBool(*b)),
                        LiteralPattern::Float(s) => {
                            if matches!(subj_ty, IrType::F32) {
                                fb.emit(Instruction::ConstF32(s.parse().unwrap_or(0.0)))
                            } else {
                                fb.emit(Instruction::ConstF64(s.parse().unwrap_or(0.0)))
                            }
                        }
                        LiteralPattern::Int(_) => {
                            let disc = literal_discriminant(lit);
                            fb.emit(match subj_ty {
                                IrType::I8 => Instruction::ConstI8(disc as i8),
                                IrType::I16 => Instruction::ConstI16(disc as i16),
                                IrType::I32 => Instruction::ConstI32(disc as i32),
                                IrType::I64 => Instruction::ConstI64(disc),
                                IrType::U8 => Instruction::ConstU8(disc as u8),
                                IrType::U16 => Instruction::ConstU16(disc as u16),
                                IrType::U32 => Instruction::ConstU32(disc as u32),
                                IrType::U64 => Instruction::ConstU64(disc as u64),
                                IrType::Usize => Instruction::ConstUsize(disc as usize),
                                _ => Instruction::ConstI64(disc),
                            })
                        }
                    };
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
                    .position(|b| {
                        b.instructions.iter().any(|(vid, _)| *vid == Some(subj_v))
                            || b.params.iter().any(|(vid, _)| *vid == subj_v)
                    })
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

        Expr::Array { elems, .. } => {
            let elem_vals: Vec<ValueId> = elems
                .iter()
                .map(|e| lower_expr(fb, e, hoisted, ctx))
                .collect::<Result<_, _>>()?;
            let return_type = fb
                .type_map
                .get(&expr.id())
                .map(|m| fb.mono_to_ir(m))
                .unwrap_or(IrType::Unit);
            let ret_v = fb.emit(Instruction::CallNamed(Box::new(crate::CallNamedData {
                name: "array_literal".into(),
                args: elem_vals.clone(),
                return_type,
            })));
            for elem in &elem_vals {
                // Skip values bound by `let` — the block cleanup handles
                // their Release and emitting another would double-free.
                if !fb.locals.values().any(|v| v == elem)
                    && let Some(ty) = fb.value_types.get(elem)
                    && ty.is_heap_type()
                {
                    fb.emit(Instruction::Release(*elem));
                }
            }
            Ok(ret_v)
        }

        Expr::Tuple { elems, .. } => {
            let elem_vals: Vec<ValueId> = elems
                .iter()
                .map(|e| lower_expr(fb, e, hoisted, ctx))
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
            let mut sorted: Vec<_> = fields
                .iter()
                .map(|f| lower_expr(fb, f.value, hoisted, ctx).map(|v| (f, v)))
                .collect::<Result<Vec<_>, _>>()?;
            // Sort by field name so field indices match the
            // alphabetical ordering that field_index_of returns
            // (derived from BTreeMap keys).
            sorted.sort_by(|(a, _), (b, _)| a.name.cmp(b.name));
            let field_vals: Vec<ValueId> = sorted.into_iter().map(|(_, v)| v).collect();
            Ok(fb.emit(Instruction::RecordAlloc(Box::new(RecordAllocData {
                type_name: "anon".into(),
                fields: field_vals,
            }))))
        }

        Expr::FieldAccess { object, field, .. } => {
            let obj_v = lower_expr(fb, object, hoisted, ctx)?;
            // Compute the field index from the object's inferred record type.
            let field_index = field_index_of(fb, obj_v, object, field);
            let fv = fb.emit(Instruction::RecordGet {
                record: obj_v,
                field: (*field).into(),
                field_index,
            });
            if let Some(ty) = fb.value_types.get(&fv)
                && ty.is_heap_type()
            {
                fb.emit(Instruction::Retain(fv));
            }
            if let Some(ty) = fb.value_types.get(&obj_v)
                && ty.is_heap_type()
            {
                fb.emit(Instruction::Release(obj_v));
            }
            Ok(fv)
        }

        Expr::Index { array, index, .. } => {
            let arr_v = lower_expr(fb, array, hoisted, ctx)?;
            let idx_v = lower_expr(fb, index, hoisted, ctx)?;
            let ev = fb.emit(Instruction::ArrayGet {
                array: arr_v,
                index: idx_v,
            });
            if let Some(ty) = fb.value_types.get(&ev)
                && ty.is_heap_type()
            {
                fb.emit(Instruction::Retain(ev));
            }
            if let Some(ty) = fb.value_types.get(&arr_v)
                && ty.is_heap_type()
            {
                fb.emit(Instruction::Release(arr_v));
            }
            if let Some(ty) = fb.value_types.get(&idx_v)
                && ty.is_heap_type()
            {
                fb.emit(Instruction::Release(idx_v));
            }
            Ok(ev)
        }

        Expr::Template { parts, .. } => {
            let mut part_vals = Vec::new();
            for part in parts {
                match part {
                    TemplatePart::Str(s) => {
                        part_vals.push(fb.emit(Instruction::ConstStr((*s).into())));
                    }
                    TemplatePart::Expr(e) => {
                        part_vals.push(lower_expr(fb, e, hoisted, ctx)?);
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
                .filter(|n| fb.locals.contains_key(n.as_str()))
                .collect();

            let inner_name: SmolStr =
                format!("{}_lambda_{}", fb.func.name, fb.func.next_value_id).into();

            // Determine the body return type from the type map.
            let body_ret_ty = fb.expr_type(body.id());

            let mut inner_fb = FunctionBuilder::new(
                inner_name.clone(),
                body_ret_ty,
                fb.globals,
                fb.value_globals,
                fb.type_map,
                fb.tag_variants,
                fb.type_args.clone(),
            );

            // EVERY function needs closure_env as arg 0
            let env_val = inner_fb.alloc_value();
            inner_fb
                .func
                .params
                .push((env_val, "closure_env".into(), IrType::I64));
            inner_fb.value_types.insert(env_val, IrType::I64);

            // Capture params — emit ClosureGet from env pointer with 8-byte offsets.
            let mut cap_offset: u32 = 16; // Skip ref_count(8) + func_ptr(8)
            for cap in &captures {
                let cap_ty = fb
                    .lookup(cap.as_str())
                    .and_then(|cv| {
                        fb.value_types.get(&cv).cloned().or_else(|| {
                            fb.func
                                .params
                                .iter()
                                .find(|(vid, _, _)| *vid == cv)
                                .map(|(_, _, ty)| ty.clone())
                        })
                    })
                    .unwrap_or(IrType::I32);
                let v = inner_fb.emit(Instruction::ClosureGet {
                    env: env_val,
                    offset: cap_offset,
                    ty: cap_ty.clone(),
                });
                inner_fb.value_types.insert(v, cap_ty);
                inner_fb.bind(cap.clone(), v);
                cap_offset += 8;
            }
            // Declared params — resolve type from annotation, type_map, or default I32.
            // Look up the lambda's MonoType::Func to extract param types when
            // the inner lambda lacks explicit type annotations.
            let lambda_mono = fb.type_map.get(&expr.id());
            for (pidx, p) in params.iter().enumerate() {
                let v = inner_fb.alloc_value();
                let param_ty =
                    p.ty.and_then(|ann| typechecker::infer::type_expr_to_mono(ann).ok())
                        .as_ref()
                        .map(|t| fb.mono_to_ir(t))
                        .or_else(|| {
                            lambda_mono.and_then(|mono| match mono {
                                MonoType::Func { params: ptys, .. } => {
                                    ptys.get(pidx).map(|t| fb.mono_to_ir(t))
                                }
                                _ => None,
                            })
                        })
                        .unwrap_or(IrType::I32);
                inner_fb
                    .func
                    .params
                    .push((v, p.name.into(), param_ty.clone()));
                inner_fb.value_types.insert(v, param_ty);
                inner_fb.bind(p.name.into(), v);
            }

            let body_v = lower_expr(&mut inner_fb, body, hoisted, ctx)?;
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

        Expr::Application { func, args, .. } => {
            let arg_vals: Vec<ValueId> = args
                .iter()
                .map(|a| lower_expr(fb, a, hoisted, ctx))
                .collect::<Result<_, _>>()?;
            let return_type = fb
                .type_map
                .get(&expr.id())
                .map(|m| fb.mono_to_ir(m))
                .unwrap_or(IrType::Unit);
            match func {
                Expr::Ident(_, name, _) => {
                    if let Some((tag_type, disc, _)) = find_tag_constructor(name, fb.tag_variants) {
                        Ok(
                            fb.emit(Instruction::TagConstruct(Box::new(TagConstructData {
                                type_name: tag_type.into(),
                                variant: (*name).into(),
                                discriminant: disc,
                                payload: arg_vals,
                            }))),
                        )
                    } else if fb.value_globals.contains::<str>(name) {
                        // Value-defined global (e.g. `let t = mk(5)`): evaluate it
                        // to get the closure value, then call it with args.
                        let callee = lower_expr(fb, func, hoisted, ctx)?;
                        let ret_v = fb.emit(Instruction::CallIndirect(Box::new(
                            crate::CallIndirectData {
                                callee,
                                args: arg_vals.clone(),
                                return_type,
                            },
                        )));
                        if let Some(ty) = fb.value_types.get(&callee)
                            && ty.is_heap_type()
                        {
                            fb.emit(Instruction::Release(callee));
                        }
                        for arg in &arg_vals {
                            if let Some(ty) = fb.value_types.get(arg)
                                && ty.is_heap_type()
                            {
                                fb.emit(Instruction::Release(*arg));
                            }
                        }
                        Ok(ret_v)
                    } else if fb.lookup(name).is_some() {
                        // Local variable or parameter used in call position.
                        // Evaluate it to get the closure value, then call via CallIndirect.
                        let callee = lower_expr(fb, func, hoisted, ctx)?;
                        let ret_v = fb.emit(Instruction::CallIndirect(Box::new(
                            crate::CallIndirectData {
                                callee,
                                args: arg_vals.clone(),
                                return_type,
                            },
                        )));
                        if let Some(ty) = fb.value_types.get(&callee)
                            && ty.is_heap_type()
                        {
                            fb.emit(Instruction::Release(callee));
                        }
                        for arg in &arg_vals {
                            if let Some(ty) = fb.value_types.get(arg)
                                && ty.is_heap_type()
                            {
                                fb.emit(Instruction::Release(*arg));
                            }
                        }
                        Ok(ret_v)
                    } else {
                        let qualified = qualify_method_name(
                            name,
                            args.first().and_then(|a| fb.type_map.get(&a.id())),
                        );
                        let mangled = resolve_and_queue_global(&qualified, func.id(), fb, ctx);
                        let ret_v =
                            fb.emit(Instruction::CallNamed(Box::new(crate::CallNamedData {
                                name: mangled.into(),
                                args: arg_vals.clone(),
                                return_type,
                            })));
                        for arg in &arg_vals {
                            if let Some(ty) = fb.value_types.get(arg)
                                && ty.is_heap_type()
                            {
                                fb.emit(Instruction::Release(*arg));
                            }
                        }
                        Ok(ret_v)
                    }
                }
                _ => {
                    let callee = lower_expr(fb, func, hoisted, ctx)?;
                    let ret_v = fb.emit(Instruction::CallIndirect(Box::new(
                        crate::CallIndirectData {
                            callee,
                            args: arg_vals.clone(),
                            return_type,
                        },
                    )));
                    if let Some(ty) = fb.value_types.get(&callee)
                        && ty.is_heap_type()
                    {
                        fb.emit(Instruction::Release(callee));
                    }
                    for arg in &arg_vals {
                        if let Some(ty) = fb.value_types.get(arg)
                            && ty.is_heap_type()
                        {
                            fb.emit(Instruction::Release(*arg));
                        }
                    }
                    Ok(ret_v)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers for discriminant and field-index resolution
// ---------------------------------------------------------------------------

/// Returns the discriminant value for a literal pattern arm.
/// Strip a known integer type suffix (like "u64", "i32", "usize") from a literal
/// string, returning the numeric portion (with any hex/octal/binary prefix intact).
fn strip_int_suffix(text: &str) -> &str {
    if let Some(s) = text.strip_suffix("i64") {
        s
    } else if let Some(s) = text.strip_suffix("i32") {
        s
    } else if let Some(s) = text.strip_suffix("i16") {
        s
    } else if let Some(s) = text.strip_suffix("i8") {
        s
    } else if let Some(s) = text.strip_suffix("u64") {
        s
    } else if let Some(s) = text.strip_suffix("u32") {
        s
    } else if let Some(s) = text.strip_suffix("u16") {
        s
    } else if let Some(s) = text.strip_suffix("u8") {
        s
    } else if let Some(s) = text.strip_suffix("usize") {
        s
    } else {
        text
    }
}

/// Parse a numeric string (after suffix removal) into an i64, handling hex/octal/binary.
fn parse_numeric_i64(text: &str) -> i64 {
    if let Some(rest) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        i64::from_str_radix(rest, 16).unwrap_or(0)
    } else if let Some(rest) = text.strip_prefix("0o").or_else(|| text.strip_prefix("0O")) {
        i64::from_str_radix(rest, 8).unwrap_or(0)
    } else if let Some(rest) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
        i64::from_str_radix(rest, 2).unwrap_or(0)
    } else {
        text.parse::<i64>().unwrap_or(0)
    }
}

/// Parse a numeric string into the given unsigned type, handling hex/octal/binary.
macro_rules! parse_numeric_as {
    ($text:expr, $ty:ty) => {{
        let t = $text;
        if let Some(rest) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
            <$ty>::from_str_radix(rest, 16).unwrap_or(0)
        } else if let Some(rest) = t.strip_prefix("0o").or_else(|| t.strip_prefix("0O")) {
            <$ty>::from_str_radix(rest, 8).unwrap_or(0)
        } else if let Some(rest) = t.strip_prefix("0b").or_else(|| t.strip_prefix("0B")) {
            <$ty>::from_str_radix(rest, 2).unwrap_or(0)
        } else {
            t.parse::<$ty>().unwrap_or(0)
        }
    }};
}

fn literal_discriminant(lit: &ast::ast::LiteralPattern<'_>) -> i64 {
    match lit {
        ast::ast::LiteralPattern::Bool(true) => 1,
        ast::ast::LiteralPattern::Bool(false) => 0,
        ast::ast::LiteralPattern::Int(s) => parse_numeric_i64(strip_int_suffix(s)),
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

/// Search all tag types for a variant with the given name.
/// Returns the tag type name, the variant discriminant, and the payload template.
fn find_tag_constructor<'a>(
    name: &str,
    tag_variants: &'a TagVariants,
) -> Option<(&'a str, u32, &'a Vec<MonoType>)> {
    for (type_name, variants) in tag_variants {
        for (idx, (vname, payload)) in variants.iter().enumerate() {
            if vname == name {
                return Some((type_name.as_str(), idx as u32, payload));
            }
        }
    }
    None
}

/// Returns the field index of `field` in the record type of `object`.
fn field_index_of(fb: &FunctionBuilder<'_>, obj_v: ValueId, object: &Expr<'_>, field: &str) -> u32 {
    if let Some(IrType::Record(rt)) = fb.value_types.get(&obj_v) {
        return rt
            .fields
            .iter()
            .position(|(k, _)| k.as_str() == field)
            .unwrap_or(0) as u32;
    }
    if let Some(obj_v) = resolve_ident_value_id(fb, object)
        && let Some(IrType::Record(rt)) = fb.value_types.get(&obj_v)
    {
        return rt
            .fields
            .iter()
            .position(|(k, _)| k.as_str() == field)
            .unwrap_or(0) as u32;
    }
    if let Some(MonoType::Record(fields)) = fb.type_map.get(&object.id()) {
        return fields.keys().position(|k| k.as_str() == field).unwrap_or(0) as u32;
    }
    if let Expr::Ident(_, name, _) = object
        && let Some(p) = fb
            .func
            .params
            .iter()
            .find(|(_, pname, _)| pname.as_str() == *name)
        && let IrType::Record(rt) = &p.2
    {
        return rt
            .fields
            .iter()
            .position(|(k, _)| k.as_str() == field)
            .unwrap_or(0) as u32;
    }
    0
}

fn resolve_ident_value_id(fb: &FunctionBuilder<'_>, expr: &Expr<'_>) -> Option<ValueId> {
    if let Expr::Ident(_, name, _) = expr {
        fb.lookup(name)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Statement and pattern lowering
// ---------------------------------------------------------------------------

fn lower_stmt<'src>(
    fb: &mut FunctionBuilder<'_>,
    stmt: &Stmt<'src>,
    hoisted: &mut Vec<IrFunction>,
    ctx: &mut MonoCtx<'_, 'src>,
) -> Result<(), LowerError> {
    match stmt {
        Stmt::Let { pattern, value } => {
            let v = lower_expr(fb, value, hoisted, ctx)?;
            bind_pattern_local(fb, pattern, v);
            if let Some(ty) = fb.value_types.get(&v)
                && ty.is_heap_type()
            {
                fb.emit(Instruction::Release(v));
            }
        }
        Stmt::Expr(e) => {
            let v = lower_expr(fb, e, hoisted, ctx)?;
            if let Some(ty) = fb.value_types.get(&v)
                && ty.is_heap_type()
            {
                fb.emit(Instruction::Release(v));
            }
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
        Pattern::Binding(_, name, _) => {
            if let Some(ty) = fb.value_types.get(&scrutinee)
                && ty.is_heap_type()
            {
                fb.emit(Instruction::Retain(scrutinee));
            }
            fb.bind((*name).into(), scrutinee);
        }
        Pattern::Wildcard(_, _) | Pattern::Literal(_, _, _) => {}
        Pattern::Constructor { name, fields, .. } => {
            let disc = subj_tag_discriminant(fb, scrutinee, name);
            for (i, p) in fields.iter().enumerate() {
                let fv = fb.emit(Instruction::TagGet(Box::new(TagGetData {
                    value: scrutinee,
                    index: i as u32,
                    discriminant: disc,
                })));
                lower_pattern(fb, p, fv)?;
            }
        }
        Pattern::Tuple { patterns, .. } => {
            for (i, p) in patterns.iter().enumerate() {
                let fv = fb.emit(Instruction::TagGet(Box::new(TagGetData {
                    value: scrutinee,
                    index: i as u32,
                    discriminant: 0,
                })));
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
                    if let Some(ty) = fb.value_types.get(&fv)
                        && ty.is_heap_type()
                    {
                        fb.emit(Instruction::Retain(fv));
                    }
                    fb.bind(f.name.into(), fv);
                }
            }
        }
    }
    Ok(())
}

fn bind_pattern_local<'src>(fb: &mut FunctionBuilder<'_>, pat: &Pattern<'src>, v: ValueId) {
    match pat {
        Pattern::Binding(_, name, _) => {
            if let Some(ty) = fb.value_types.get(&v)
                && ty.is_heap_type()
            {
                fb.emit(Instruction::Retain(v));
            }
            fb.bind((*name).into(), v);
        }
        Pattern::Wildcard(..) | Pattern::Literal(..) => {}
        Pattern::Tuple { patterns, .. } => {
            for (i, p) in patterns.iter().enumerate() {
                let fv = fb.emit(Instruction::TagGet(Box::new(TagGetData {
                    value: v,
                    index: i as u32,
                    discriminant: 0,
                })));
                bind_pattern_local(fb, p, fv);
            }
        }
        Pattern::Constructor { name, fields, .. } => {
            let disc = subj_tag_discriminant(fb, v, name);
            for (i, p) in fields.iter().enumerate() {
                let fv = fb.emit(Instruction::TagGet(Box::new(TagGetData {
                    value: v,
                    index: i as u32,
                    discriminant: disc,
                })));
                bind_pattern_local(fb, p, fv);
            }
        }
        Pattern::Record { fields, .. } => {
            for (i, f) in fields.iter().enumerate() {
                let fv = fb.emit(Instruction::RecordGet {
                    record: v,
                    field: f.name.into(),
                    field_index: i as u32,
                });
                if let Some(p) = f.pattern {
                    bind_pattern_local(fb, p, fv);
                } else {
                    if let Some(ty) = fb.value_types.get(&fv)
                        && ty.is_heap_type()
                    {
                        fb.emit(Instruction::Retain(fv));
                    }
                    fb.bind(f.name.into(), fv);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Literal parsing
// ---------------------------------------------------------------------------

fn parse_int_literal(text: &str) -> Instruction {
    if let Some(s) = text.strip_suffix("i64") {
        return Instruction::ConstI64(parse_numeric_i64(s));
    }
    if let Some(s) = text.strip_suffix("i32") {
        return Instruction::ConstI32(parse_numeric_i64(s) as i32);
    }
    if let Some(s) = text.strip_suffix("i16") {
        return Instruction::ConstI16(parse_numeric_i64(s) as i16);
    }
    if let Some(s) = text.strip_suffix("i8") {
        return Instruction::ConstI8(parse_numeric_i64(s) as i8);
    }
    if let Some(s) = text.strip_suffix("u64") {
        return Instruction::ConstU64(parse_numeric_as!(s, u64));
    }
    if let Some(s) = text.strip_suffix("u32") {
        return Instruction::ConstU32(parse_numeric_as!(s, u32));
    }
    if let Some(s) = text.strip_suffix("u16") {
        return Instruction::ConstU16(parse_numeric_as!(s, u16));
    }
    if let Some(s) = text.strip_suffix("u8") {
        return Instruction::ConstU8(parse_numeric_as!(s, u8));
    }
    if let Some(s) = text.strip_suffix("usize") {
        return Instruction::ConstUsize(parse_numeric_as!(s, usize));
    }
    // No suffix: default to I32, but handle hex/octal/binary
    Instruction::ConstI32(parse_numeric_i64(text) as i32)
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

#[allow(clippy::too_many_arguments)]
fn lower_decl_monomorphized<'src>(
    mangled_name: String,
    decl: &Decl<'src>,
    globals: &HashSet<SmolStr>,
    value_globals: &HashSet<SmolStr>,
    type_map: &HashMap<NodeId, MonoType>,
    tag_variants: &TagVariants,
    type_args: HashMap<typechecker::TypeId, IrType>,
    module: &mut IrModule,
    ctx: &mut MonoCtx<'_, 'src>,
) -> Result<(), LowerError> {
    match decl {
        Decl::Bind {
            value, id: decl_id, ..
        } => {
            let ret_ty = type_map
                .get(decl_id)
                .map(|t| match (value, t) {
                    (Expr::Lambda { .. }, MonoType::Func { ret, .. }) => {
                        mono_to_ir_inner(ret, Some(tag_variants), &type_args)
                    }
                    (_, other) => mono_to_ir_inner(other, Some(tag_variants), &type_args),
                })
                .unwrap_or(IrType::I32);
            let mut hoisted = Vec::new();

            match value {
                Expr::Lambda { params, body, .. } => {
                    let mut fb = FunctionBuilder::new(
                        mangled_name.into(),
                        ret_ty,
                        globals,
                        value_globals,
                        type_map,
                        tag_variants,
                        type_args,
                    );
                    let env_val = fb.alloc_value();
                    fb.func
                        .params
                        .push((env_val, "closure_env".into(), IrType::I64));
                    fb.value_types.insert(env_val, IrType::I64);
                    for p in params.iter() {
                        let v = fb.alloc_value();
                        let param_ty = p
                            .ty
                            .and_then(|ann| typechecker::infer::type_expr_to_mono(ann).ok())
                            .as_ref()
                            .map(|t| fb.mono_to_ir(t))
                            .or_else(|| {
                                type_map.get(decl_id).and_then(|mono| match mono {
                                    MonoType::Func { params: ptys, .. } => {
                                        let idx = params.iter().position(|q| q.name == p.name)?;
                                        Some(fb.mono_to_ir(&ptys[idx]))
                                    }
                                    _ => None,
                                })
                            })
                            .unwrap_or(IrType::I32);
                        fb.func.params.push((v, p.name.into(), param_ty.clone()));
                        fb.value_types.insert(v, param_ty);
                        fb.bind(p.name.into(), v);
                    }
                    let body_v = lower_expr(&mut fb, body, &mut hoisted, ctx)?;
                    fb.set_terminator(Terminator::Return(body_v));
                    module.decls.push(IrDecl::Function(fb.func));
                }
                other => {
                    let mut fb = FunctionBuilder::new(
                        mangled_name.into(),
                        ret_ty,
                        globals,
                        value_globals,
                        type_map,
                        tag_variants,
                        type_args,
                    );
                    let env_val = fb.alloc_value();
                    fb.func
                        .params
                        .push((env_val, "closure_env".into(), IrType::I64));
                    fb.value_types.insert(env_val, IrType::I64);
                    let v = lower_expr(&mut fb, other, &mut hoisted, ctx)?;
                    fb.set_terminator(Terminator::Return(v));
                    module.decls.push(IrDecl::Function(fb.func));
                }
            }

            for f in hoisted {
                module.decls.push(IrDecl::Function(f));
            }
            Ok(())
        }
        _ => Ok(()),
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

    let mut globals: HashSet<SmolStr> = typed
        .ast
        .decls
        .iter()
        .filter_map(|d| match d {
            Decl::Bind { name, .. } => Some((*name).into()),
            _ => None,
        })
        .collect();

    // Add known builtin names so the lowerer emits CallNamed for them
    // instead of returning Unbound. These are resolved at runtime by
    // the BuiltinRegistry.
    let builtin_names = [
        "id",
        "const",
        "flip",
        "compose",
        "pipe",
        "apply",
        "map",
        "filter",
        "fold",
        "flatMap",
        "concat",
        "prepend",
        "len",
        "head",
        "tail",
        "split",
        "trim",
        "parse_i32",
        "println",
        "print",
        "read_line",
        "readFile",
        "to_i64",
        "to_i32",
        "to_f64",
        "to_str",
        "drop",
        "take",
        "sqrt",
        "unwrap",
        "unwrap_or",
    ];
    for name in builtin_names {
        globals.insert((*name).into());
    }

    // Track globals defined with a non-lambda expression (e.g. `let t = mk(5)`).
    // These are closure VALUES, not function definitions. When used as the
    // func position of an application, they must be evaluated first (via
    // CallNamed with no args) and then the result called via CallIndirect,
    // rather than emitting CallNamed(name, args) directly.
    let value_globals: HashSet<SmolStr> = typed
        .ast
        .decls
        .iter()
        .filter_map(|d| match d {
            Decl::Bind { name, value, .. } if !matches!(value, Expr::Lambda { .. }) => {
                Some((*name).into())
            }
            _ => None,
        })
        .collect();

    // First, scan and lower imports and type aliases.
    for decl in &typed.ast.decls {
        match decl {
            Decl::Use { path, .. } => {
                module.imports.push(path.join("::").into());
            }
            Decl::TypeAlias {
                name, id: decl_id, ..
            } => {
                if let Some(mono) = typed.type_map.get(decl_id) {
                    module.decls.push(IrDecl::TypeAlias {
                        name: (*name).into(),
                        ty: mono_to_ir_inner(mono, Some(&typed.tag_variants), &HashMap::new()),
                    });
                }
            }
            Decl::Bind { .. } => {}
        }
    }

    // Build the map of global declarations for user-defined bindings.
    let mut global_decls = HashMap::new();
    for decl in &typed.ast.decls {
        if let Decl::Bind { name, .. } = decl {
            global_decls.insert(*name, decl);
        }
    }

    let mut queue = Vec::new();
    let mut generated = HashSet::new();

    // Populate initial roots: queue all user-defined bindings under their original names.
    for (&name, &decl) in &global_decls {
        let mangled = name.to_string();
        if !generated.contains(&mangled) {
            generated.insert(mangled.clone());
            queue.push((mangled, decl, HashMap::new()));
        }
    }

    while !queue.is_empty() {
        let (mangled_name, decl, type_args) = queue.remove(0);
        lower_decl_monomorphized(
            mangled_name,
            decl,
            &globals,
            &value_globals,
            &typed.type_map,
            &typed.tag_variants,
            type_args,
            &mut module,
            &mut MonoCtx {
                queue: &mut queue,
                generated: &mut generated,
                global_decls: &global_decls,
                env: &typed.env,
            },
        )?;
    }

    module.tag_variants = typed.tag_variants.clone();
    Ok(module)
}
