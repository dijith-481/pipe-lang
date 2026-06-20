use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use ast::SmolStr;
use ast::ast::{BinOp, Decl, Expr, LiteralPattern, MatchArm, Pattern, Stmt, TypeExpr, UnaryOp};
use ast::span::Span;
use ast::SmolStr;

use crate::env::TypeEnv;
use crate::error::TypeError;
use crate::types::{MonoType, PolyType, TypeId};
use crate::unify::{Substitution, unify};

// ---------------------------------------------------------------------------
// Free type variables
// ---------------------------------------------------------------------------

fn free_vars_mono(ty: &MonoType, out: &mut HashSet<TypeId>) {
    match ty {
        MonoType::Var(id) => {
            out.insert(*id);
        }
        MonoType::Array(inner) => free_vars_mono(inner, out),
        MonoType::Func { params, ret } => {
            params.iter().for_each(|p| free_vars_mono(p, out));
            free_vars_mono(ret, out);
        }
        MonoType::Record(fields) => fields.values().for_each(|t| free_vars_mono(t, out)),
        MonoType::Tag { payload, .. } => payload.iter().for_each(|t| free_vars_mono(t, out)),
        MonoType::Effect(inner) => free_vars_mono(inner, out),
        _ => {}
    }
}

fn free_vars_env(env: &TypeEnv) -> HashSet<TypeId> {
    let mut out = HashSet::new();
    for pt in env.all_types() {
        let mut body_free = HashSet::new();
        free_vars_mono(&pt.body, &mut body_free);
        for q in &pt.quantified {
            body_free.remove(q);
        }
        out.extend(body_free);
    }
    out
}

// ---------------------------------------------------------------------------
// Generalize & instantiate
// ---------------------------------------------------------------------------

/// Closes over all free type variables in `ty` that are not free in `env`.
pub fn generalize(env: &TypeEnv, sub: &mut Substitution, ty: &MonoType) -> PolyType {
    let ty = sub.apply(ty);
    let mut mono_free = HashSet::new();
    free_vars_mono(&ty, &mut mono_free);
    let env_free = free_vars_env(env);
    let quantified: Vec<TypeId> = mono_free.difference(&env_free).copied().collect();
    PolyType::poly(quantified, ty)
}

/// Freshens all quantified variables at a call site.
pub fn instantiate(env: &mut TypeEnv, sub: &mut Substitution, poly: &PolyType) -> MonoType {
    let mapping: HashMap<TypeId, MonoType> = poly
        .quantified
        .iter()
        .map(|&q| {
            let fresh = env.fresh_var();
            sub.ensure_key(fresh);
            (q, MonoType::Var(fresh))
        })
        .collect();
    apply_mapping(&poly.body, &mapping)
}

fn apply_mapping(ty: &MonoType, m: &HashMap<TypeId, MonoType>) -> MonoType {
    match ty {
        MonoType::Var(id) => m.get(id).cloned().unwrap_or_else(|| ty.clone()),
        MonoType::Array(inner) => MonoType::Array(Rc::new(apply_mapping(inner, m))),
        MonoType::Func { params, ret } => MonoType::Func {
            params: params.iter().map(|p| apply_mapping(p, m)).collect(),
            ret: Rc::new(apply_mapping(ret, m)),
        },
        MonoType::Record(fields) => MonoType::Record(Rc::new(
            fields
                .iter()
                .map(|(n, t)| (n.clone(), apply_mapping(t, m)))
                .collect(),
        )),
        MonoType::Tag { name, payload } => MonoType::Tag {
            name: name.clone(),
            payload: payload.iter().map(|t| apply_mapping(t, m)).collect(),
        },
        MonoType::Effect(inner) => MonoType::Effect(Box::new(apply_mapping(inner, m))),
        _ => ty.clone(),
    }
}

// ---------------------------------------------------------------------------
// TypeExpr → MonoType
// ---------------------------------------------------------------------------

/// Collects generic type parameter names from a type expression.
/// Recognizes single lowercase letter names as generic params.
fn collect_generic_names<'a>(te: &'a TypeExpr<'a>, names: &mut Vec<&'a str>) {
    match te {
        TypeExpr::Named(name, _) => {
            if name.len() == 1 && name.chars().all(|c| c.is_ascii_lowercase()) {
                names.push(*name);
            }
        }
        TypeExpr::Function { from, to, .. } => {
            collect_generic_names(from, names);
            collect_generic_names(to, names);
        }
        TypeExpr::Tuple { types, .. } => {
            for t in types {
                collect_generic_names(t, names);
            }
        }
        TypeExpr::Record { fields, .. } => {
            for f in fields {
                collect_generic_names(f.ty, names);
            }
        }
        TypeExpr::Apply { func, arg, .. } => {
            collect_generic_names(func, names);
            collect_generic_names(arg, names);
        }
        TypeExpr::Sum { variants, .. } => {
            for v in variants {
                for f in &v.fields {
                    collect_generic_names(f, names);
                }
            }
        }
    }
}

/// Internal annotation resolver with generic param support.
fn type_expr_to_mono_inner(
    env: &TypeEnv,
    te: &TypeExpr<'_>,
    generics: &HashMap<SmolStr, MonoType>,
) -> Result<MonoType, TypeError> {
    match te {
        TypeExpr::Named(name, span) => {
            if let Some(subst) = generics.get(*name) {
                return Ok(subst.clone());
            }
            match *name {
                "i8" => Ok(MonoType::I8),
                "i16" => Ok(MonoType::I16),
                "i32" => Ok(MonoType::I32),
                "i64" => Ok(MonoType::I64),
                "u8" => Ok(MonoType::U8),
                "u16" => Ok(MonoType::U16),
                "u32" => Ok(MonoType::U32),
                "u64" => Ok(MonoType::U64),
                "usize" => Ok(MonoType::Usize),
                "f32" => Ok(MonoType::F32),
                "f64" => Ok(MonoType::F64),
                "bool" => Ok(MonoType::Bool),
                "str" => Ok(MonoType::Str),
                "()" => Ok(MonoType::Unit),
                _ => {
                    // Try to resolve user-defined type from env
                    if let Some(poly) = env.lookup(name) {
                        Ok(poly.body.clone())
                    } else {
                        Err(TypeError::UnboundVariable {
                            name: (*name).to_string(),
                            span: *span,
                        })
                    }
                }
            }
        }
        TypeExpr::Function { from, to, .. } => {
            let from_resolved = type_expr_to_mono_inner(env, from, generics)?;
            // Multi-param fix: unwrap Tuple into individual params
            let params = match &from_resolved {
                MonoType::Tag { name, payload } if name.as_str() == "Tuple" => {
                    payload.to_vec()
                }
                _ => vec![from_resolved],
            };
            Ok(MonoType::Func {
                params: Rc::from(params),
                ret: Rc::new(type_expr_to_mono_inner(env, to, generics)?),
            })
        }
        TypeExpr::Tuple { types, span: _ } => {
            let payload: Result<Vec<MonoType>, _> =
                types.iter().map(|t| type_expr_to_mono_inner(env, t, generics)).collect();
            Ok(MonoType::Tag {
                name: "Tuple".into(),
                payload: Rc::from(payload?.as_slice()),
            })
        }
        TypeExpr::Record { fields, .. } => {
            let mut map = BTreeMap::new();
            for f in fields {
                map.insert(f.name.into(), type_expr_to_mono_inner(env, f.ty, generics)?);
            }
            Ok(MonoType::Record(Rc::new(map)))
        }
        TypeExpr::Apply { func, arg, span } => {
            let mut args = vec![type_expr_to_mono_inner(env, arg, generics)?];
            let mut current = func;
            let base_name = loop {
                match current {
                    TypeExpr::Named(n, _) => break *n,
                    TypeExpr::Apply {
                        func: inner,
                        arg: inner_arg,
                        ..
                    } => {
                        args.push(type_expr_to_mono_inner(env, inner_arg, generics)?);
                        current = inner;
                    }
                    _ => {
                        return Err(TypeError::UnboundVariable {
                            name: "complex type application".to_string(),
                            span: *span,
                        });
                    }
                }
            };
            args.reverse();
            if base_name == "Array" && args.len() == 1 {
                Ok(MonoType::Array(Rc::new(args.into_iter().next().unwrap())))
            } else if base_name == "Effect" && args.len() == 1 {
                Ok(MonoType::Effect(Box::new(args.into_iter().next().unwrap())))
            } else {
                Ok(MonoType::Tag {
                    name: base_name.into(),
                    payload: Rc::from(args.as_slice()),
                })
            }
        }
        TypeExpr::Sum { span, .. } => Err(TypeError::UnboundVariable {
            name: "anonymous sum type".to_string(),
            span: *span,
        }),
    }
}

/// Converts a syntax-level type annotation into a [`MonoType`], resolving
/// user-defined type names from the environment.
///
/// # Errors
///
/// Returns [`TypeError::UnboundVariable`] for unknown type names.
pub fn type_expr_to_mono_with_env(
    env: &TypeEnv,
    te: &TypeExpr<'_>,
) -> Result<MonoType, TypeError> {
    type_expr_to_mono_inner(env, te, &HashMap::new())
}

/// Resolves a type expression, auto-detecting single-letter generic type
/// variables (e.g. `a`, `b`) and binding them to fresh type variables.
pub fn type_expr_to_mono_with_generics(
    env: &mut TypeEnv,
    te: &TypeExpr<'_>,
) -> Result<MonoType, TypeError> {
    let mut names = Vec::new();
    collect_generic_names(te, &mut names);
    names.sort();
    names.dedup();
    let mut generics = HashMap::new();
    for name in &names {
        let v = env.fresh_var();
        generics.insert(SmolStr::from(*name), MonoType::Var(v));
    }
    type_expr_to_mono_inner(env, te, &generics)
}

/// Legacy wrapper — only resolves hardcoded primitive names.
/// Fails with [`TypeError::UnboundVariable`] for user-defined types.
///
/// Prefer [`type_expr_to_mono_with_env`] going forward.
pub fn type_expr_to_mono(te: &TypeExpr<'_>) -> Result<MonoType, TypeError> {
    let empty_env = TypeEnv::new();
    type_expr_to_mono_with_env(&empty_env, te)
}

// ---------------------------------------------------------------------------
// Pattern binding
// ---------------------------------------------------------------------------

fn bind_pattern<'a>(
    env: &mut TypeEnv,
    sub: &mut Substitution,
    pat: &Pattern<'a>,
) -> Result<MonoType, TypeError> {
    match pat {
        Pattern::Wildcard(_) => {
            let v = env.fresh_var();
            sub.ensure_key(v);
            Ok(MonoType::Var(v))
        }
        Pattern::Binding(name, _) => {
            let v = env.fresh_var();
            sub.ensure_key(v);
            let ty = MonoType::Var(v);
            env.insert(*name, PolyType::mono(ty.clone()));
            Ok(ty)
        }
        Pattern::Literal(lit, _span) => match lit {
            LiteralPattern::Int(text) => Ok(int_literal_type(text)),
            LiteralPattern::Float(_) => Ok(MonoType::F64),
            LiteralPattern::Str(_) => Ok(MonoType::Str),
            LiteralPattern::Bool(_) => Ok(MonoType::Bool),
        },
        Pattern::Tuple { patterns, .. } => {
            let tys: Result<Vec<_>, _> =
                patterns.iter().map(|p| bind_pattern(env, sub, p)).collect();
            Ok(MonoType::Tag {
                name: "Tuple".into(),
                payload: Rc::from(tys?.as_slice()),
            })
        }
        Pattern::Record { fields, .. } => {
            let mut map = BTreeMap::new();
            for f in fields {
                let ty = if let Some(p) = f.pattern {
                    bind_pattern(env, sub, p)?
                } else {
                    let v = env.fresh_var();
                    sub.ensure_key(v);
                    let ty = MonoType::Var(v);
                    env.insert(f.name, PolyType::mono(ty.clone()));
                    ty
                };
                map.insert(f.name.into(), ty);
            }
            Ok(MonoType::Record(Rc::new(map)))
        }
        Pattern::Constructor { name, fields, .. } => {
            // Look up the constructor in the env (e.g. Some, None, Ok, Err).
            // If found, use its return type as the parent type for unification.
            if let Some(poly) = env.lookup(name).cloned() {
                let ctor_ty = instantiate(env, sub, &poly);
                let ctor_applied = sub.apply(&ctor_ty);
                match ctor_applied {
                    MonoType::Func { params, ret } => {
                        // Constructor takes params and returns parent type.
                        // Bind each pattern field to the corresponding param type.
                        for (field_pat, param_ty) in fields.iter().zip(params.iter()) {
                            let field_ty = bind_pattern(env, sub, field_pat)?;
                            let param_applied = sub.apply(param_ty);
                            let field_applied = sub.apply(&field_ty);
                            unify(sub, &param_applied, &field_applied)?;
                        }
                        Ok(ret.as_ref().clone())
                    }
                    MonoType::Tag { .. } => {
                        // Constructor with no params (e.g. None).
                        // Return the tag type directly.
                        Ok(ctor_applied)
                    }
                    _ => {
                        // Not a constructor type, fallback to building Tag.
                        let payload: Result<Vec<_>, _> =
                            fields.iter().map(|p| bind_pattern(env, sub, p)).collect();
                        Ok(MonoType::Tag {
                            name: (*name).into(),
                            payload: Rc::from(payload?.as_slice()),
                        })
                    }
                }
            } else {
                // Constructor not in env (user-defined), build Tag from pattern.
                let payload: Result<Vec<_>, _> =
                    fields.iter().map(|p| bind_pattern(env, sub, p)).collect();
                Ok(MonoType::Tag {
                    name: (*name).into(),
                    payload: Rc::from(payload?.as_slice()),
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Literal type helpers
// ---------------------------------------------------------------------------

fn int_literal_type(text: &str) -> MonoType {
    if text.ends_with("i64") {
        return MonoType::I64;
    }
    if text.ends_with("i16") {
        return MonoType::I16;
    }
    if text.ends_with("i8") {
        return MonoType::I8;
    }
    if text.ends_with("u64") {
        return MonoType::U64;
    }
    if text.ends_with("u32") {
        return MonoType::U32;
    }
    if text.ends_with("u16") {
        return MonoType::U16;
    }
    if text.ends_with("u8") {
        return MonoType::U8;
    }
    if text.ends_with("usize") {
        return MonoType::Usize;
    }
    // "i32" suffix or bare decimal → i32
    MonoType::I32
}

fn float_literal_type(text: &str) -> MonoType {
    if text.ends_with("f32") {
        return MonoType::F32;
    }
    MonoType::F64
}

// ---------------------------------------------------------------------------
// Core inference (Algorithm W)
// ---------------------------------------------------------------------------

/// Infers the type of `expr`, recording sub-expression types in `type_map`.
///
/// Every expression's fully-resolved span→type is inserted into `type_map`
/// before returning, enabling the IR lowerer to look up any sub-expression's
/// type without a second traversal.
///
/// # Errors
///
/// Returns [`TypeError`] on type mismatches, unbound variables, or infinite types.
pub fn infer<'a>(
    env: &mut TypeEnv,
    sub: &mut Substitution,
    type_map: &mut HashMap<Span, MonoType>,
    expr: &Expr<'a>,
) -> Result<MonoType, TypeError> {
    let result = infer_inner(env, sub, type_map, expr)?;
    // Apply current substitution so the stored type is maximally resolved.
    let resolved = sub.apply(&result);
    type_map.insert(expr.span(), resolved.clone());
    Ok(resolved)
}

fn infer_inner<'a>(
    env: &mut TypeEnv,
    sub: &mut Substitution,
    type_map: &mut HashMap<Span, MonoType>,
    expr: &Expr<'a>,
) -> Result<MonoType, TypeError> {
    match expr {
        Expr::IntLiteral(text, _) => Ok(int_literal_type(text)),
        Expr::FloatLiteral(text, _) => Ok(float_literal_type(text)),
        Expr::Bool(_, _) => Ok(MonoType::Bool),
        Expr::Str(_, _) => Ok(MonoType::Str),

        Expr::Ident(name, span) => {
            let poly = env
                .lookup(name)
                .ok_or_else(|| TypeError::UnboundVariable {
                    name: (*name).to_string(),
                    span: *span,
                })?
                .clone();
            Ok(instantiate(env, sub, &poly))
        }

        Expr::Lambda {
            params,
            return_type,
            body,
            ..
        } => {
            env.push_scope();
            let param_tys: Vec<MonoType> = params
                .iter()
                .map(|p| {
                    let ty = if let Some(ann) = p.ty {
                        type_expr_to_mono_with_env(env, ann)?
                    } else {
                        let v = env.fresh_var();
                        sub.ensure_key(v);
                        MonoType::Var(v)
                    };
                    env.insert(p.name, PolyType::mono(ty.clone()));
                    Ok(ty)
                })
                .collect::<Result<_, TypeError>>()?;

            let body_ty = infer(env, sub, type_map, body)?;
            env.pop_scope();

            let ret_ty = if let Some(ann) = return_type {
                let ann_ty = type_expr_to_mono_with_env(env, ann)?;
                let body_applied = sub.apply(&body_ty);
                unify(sub, &body_applied, &ann_ty)?;
                sub.apply(&ann_ty)
            } else {
                sub.apply(&body_ty)
            };

            let resolved_params: Vec<MonoType> = param_tys.iter().map(|t| sub.apply(t)).collect();
            Ok(MonoType::Func {
                params: Rc::from(resolved_params.as_slice()),
                ret: Rc::new(ret_ty),
            })
        }

        Expr::Application { func, args, span } => {
            let func_ty = infer(env, sub, type_map, func)?;
            let arg_tys: Vec<MonoType> = args
                .iter()
                .map(|a| infer(env, sub, type_map, a))
                .collect::<Result<_, TypeError>>()?;

            let ret_var = env.fresh_var();
            sub.ensure_key(ret_var);
            let ret_ty = MonoType::Var(ret_var);
            let expected = MonoType::Func {
                params: Rc::from(arg_tys.as_slice()),
                ret: Rc::new(ret_ty.clone()),
            };
            let func_applied = sub.apply(&func_ty);
            unify(sub, &func_applied, &expected).map_err(|e| match e {
                TypeError::ArityMismatch {
                    expected: exp, got, ..
                } => TypeError::ArityMismatch {
                    expected: exp,
                    got,
                    span: *span,
                },
                other => other,
            })?;
            Ok(sub.apply(&ret_ty))
        }

        Expr::Binary {
            op,
            left,
            right,
            span,
        } => {
            let lt = infer(env, sub, type_map, left)?;
            let rt = infer(env, sub, type_map, right)?;
            let la = sub.apply(&lt);
            let ra = sub.apply(&rt);
            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                    unify(sub, &la, &ra).map_err(|_| TypeError::UnificationFailed {
                        expected: la.clone(),
                        got: ra.clone(),
                        span: *span,
                    })?;
                    Ok(sub.apply(&la))
                }
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    unify(sub, &la, &ra).map_err(|_| TypeError::UnificationFailed {
                        expected: la.clone(),
                        got: ra.clone(),
                        span: *span,
                    })?;
                    Ok(MonoType::Bool)
                }
                BinOp::And | BinOp::Or => {
                    unify(sub, &la, &MonoType::Bool).map_err(|_| TypeError::UnificationFailed {
                        expected: MonoType::Bool,
                        got: la.clone(),
                        span: *span,
                    })?;
                    unify(sub, &ra, &MonoType::Bool).map_err(|_| TypeError::UnificationFailed {
                        expected: MonoType::Bool,
                        got: ra.clone(),
                        span: *span,
                    })?;
                    Ok(MonoType::Bool)
                }
            }
        }

        Expr::Unary { op, operand, span } => {
            let ty = infer(env, sub, type_map, operand)?;
            let ta = sub.apply(&ty);
            match op {
                UnaryOp::Neg => {
                    if !ta.is_numeric() {
                        return Err(TypeError::UnificationFailed {
                            expected: MonoType::I32,
                            got: ta,
                            span: *span,
                        });
                    }
                    Ok(ta)
                }
                UnaryOp::Not => {
                    unify(sub, &ta, &MonoType::Bool).map_err(|_| TypeError::UnificationFailed {
                        expected: MonoType::Bool,
                        got: ta.clone(),
                        span: *span,
                    })?;
                    Ok(MonoType::Bool)
                }
            }
        }

        Expr::If {
            condition,
            then_branch,
            else_branch,
            span,
        } => {
            let ct = infer(env, sub, type_map, condition)?;
            let ca = sub.apply(&ct);
            unify(sub, &ca, &MonoType::Bool).map_err(|_| TypeError::UnificationFailed {
                expected: MonoType::Bool,
                got: ca.clone(),
                span: *span,
            })?;
            let tt = infer(env, sub, type_map, then_branch)?;
            let et = infer(env, sub, type_map, else_branch)?;
            let ta = sub.apply(&tt);
            let ea = sub.apply(&et);
            unify(sub, &ta, &ea).map_err(|_| TypeError::UnificationFailed {
                expected: ta.clone(),
                got: ea.clone(),
                span: *span,
            })?;
            Ok(sub.apply(&ta))
        }

        Expr::Block { stmts, result, .. } => {
            env.push_scope();
            for stmt in stmts {
                infer_stmt(env, sub, type_map, stmt)?;
            }
            let ty = infer(env, sub, type_map, result)?;
            env.pop_scope();
            Ok(sub.apply(&ty))
        }

        Expr::Match {
            subject,
            arms,
            span,
        } => {
            if arms.is_empty() {
                return Err(TypeError::NonExhaustiveMatch { span: *span });
            }
            let subj_ty = infer(env, sub, type_map, subject)?;
            let mut result_ty: Option<MonoType> = None;
            for arm in arms {
                infer_arm(env, sub, type_map, arm, &subj_ty, span, &mut result_ty)?;
            }
            Ok(sub.apply(result_ty.as_ref().unwrap()))
        }

        Expr::Array { elems, span } => {
            let elem_var = env.fresh_var();
            sub.ensure_key(elem_var);
            let elem_ty = MonoType::Var(elem_var);
            for elem in elems {
                let et = infer(env, sub, type_map, elem)?;
                let ea = sub.apply(&et);
                let va = sub.apply(&elem_ty);
                unify(sub, &ea, &va).map_err(|_| TypeError::UnificationFailed {
                    expected: va.clone(),
                    got: ea.clone(),
                    span: *span,
                })?;
            }
            Ok(MonoType::Array(Rc::new(sub.apply(&elem_ty))))
        }

        Expr::Tuple { elems, .. } => {
            let tys: Vec<MonoType> = elems
                .iter()
                .map(|e| {
                    let t = infer(env, sub, type_map, e)?;
                    Ok(sub.apply(&t))
                })
                .collect::<Result<_, TypeError>>()?;
            Ok(MonoType::Tag {
                name: "Tuple".into(),
                payload: Rc::from(tys.as_slice()),
            })
        }

        Expr::Record { fields, .. } => {
            let mut map = BTreeMap::new();
            for f in fields {
                let ft = infer(env, sub, type_map, f.value)?;
                map.insert(f.name.into(), sub.apply(&ft));
            }
            Ok(MonoType::Record(Rc::new(map)))
        }

        Expr::FieldAccess {
            object,
            field,
            span,
        } => {
            let obj_ty = infer(env, sub, type_map, object)?;
            let oa = sub.apply(&obj_ty);
            match &oa {
                MonoType::Record(fields) => {
                    fields
                        .get(*field)
                        .cloned()
                        .ok_or_else(|| TypeError::FieldNotFound {
                            field: (*field).to_string(),
                            span: *span,
                        })
                }
                MonoType::Var(_) => {
                    let fv = env.fresh_var();
                    sub.ensure_key(fv);
                    let fv_ty = MonoType::Var(fv);
                    let mut map = BTreeMap::new();
                    map.insert((*field).into(), fv_ty.clone());
                    let expected_rec = MonoType::Record(Rc::new(map));
                    unify(sub, &oa, &expected_rec).map_err(|_| TypeError::FieldNotFound {
                        field: (*field).to_string(),
                        span: *span,
                    })?;
                    Ok(sub.apply(&fv_ty))
                }
                _ => Err(TypeError::FieldNotFound {
                    field: (*field).to_string(),
                    span: *span,
                }),
            }
        }

        Expr::Template { parts, .. } => {
            for part in parts {
                if let ast::ast::TemplatePart::Expr(e) = part {
                    infer(env, sub, type_map, e)?;
                }
            }
            Ok(MonoType::Str)
        }

        Expr::Index { array, index, span } => {
            let arr_ty = infer(env, sub, type_map, array)?;
            let idx_ty = infer(env, sub, type_map, index)?;
            let ia = sub.apply(&idx_ty);
            unify(sub, &ia, &MonoType::I32)
                .or_else(|_| unify(sub, &ia, &MonoType::Usize))
                .map_err(|_| TypeError::UnificationFailed {
                    expected: MonoType::I32,
                    got: ia.clone(),
                    span: *span,
                })?;
            let elem_var = env.fresh_var();
            sub.ensure_key(elem_var);
            let elem_ty = MonoType::Var(elem_var);
            let expected_arr = MonoType::Array(Rc::new(elem_ty.clone()));
            let aa = sub.apply(&arr_ty);
            unify(sub, &aa, &expected_arr).map_err(|_| TypeError::UnificationFailed {
                expected: expected_arr.clone(),
                got: aa.clone(),
                span: *span,
            })?;
            Ok(sub.apply(&elem_ty))
        }
    }
}

fn infer_stmt<'a>(
    env: &mut TypeEnv,
    sub: &mut Substitution,
    type_map: &mut HashMap<Span, MonoType>,
    stmt: &Stmt<'a>,
) -> Result<(), TypeError> {
    match stmt {
        Stmt::Let { pattern, value } => {
            let ty = infer(env, sub, type_map, value)?;
            let ta = sub.apply(&ty);
            let poly = generalize(env, sub, &ta);
            bind_stmt_pattern(env, sub, pattern, poly)?;
        }
        Stmt::Expr(e) => {
            infer(env, sub, type_map, e)?;
        }
    }
    Ok(())
}

fn bind_stmt_pattern<'a>(
    env: &mut TypeEnv,
    sub: &mut Substitution,
    pat: &Pattern<'a>,
    poly: PolyType,
) -> Result<(), TypeError> {
    match pat {
        Pattern::Binding(name, _) => env.insert(*name, poly),
        Pattern::Wildcard(_) => {}
        _ => {
            bind_pattern(env, sub, pat)?;
        }
    }
    Ok(())
}

fn infer_arm<'a>(
    env: &mut TypeEnv,
    sub: &mut Substitution,
    type_map: &mut HashMap<Span, MonoType>,
    arm: &MatchArm<'a>,
    subj_ty: &MonoType,
    span: &Span,
    result_ty: &mut Option<MonoType>,
) -> Result<(), TypeError> {
    env.push_scope();
    let pat_ty = bind_pattern(env, sub, arm.pattern)?;
    let sa = sub.apply(subj_ty);
    let pa = sub.apply(&pat_ty);
    unify(sub, &sa, &pa).map_err(|_| TypeError::UnificationFailed {
        expected: sa.clone(),
        got: pa.clone(),
        span: *span,
    })?;
    let arm_ty = infer(env, sub, type_map, arm.body)?;
    env.pop_scope();
    let arm_applied = sub.apply(&arm_ty);
    match result_ty {
        None => *result_ty = Some(arm_applied),
        Some(prev) => {
            let pa = sub.apply(prev);
            unify(sub, &pa, &arm_applied).map_err(|_| TypeError::UnificationFailed {
                expected: pa.clone(),
                got: arm_applied.clone(),
                span: *span,
            })?;
            *result_ty = Some(sub.apply(&pa));
        }
    }
    Ok(())
}

/// Resolve a TypeExpr to MonoType, substituting generic param names with their type vars.
fn resolve_type_expr_with_env<'a>(
    env: &TypeEnv,
    te: &TypeExpr<'a>,
    params: &HashMap<&str, MonoType>,
) -> Result<MonoType, TypeError> {
    match te {
        TypeExpr::Named(name, _span) => {
            if let Some(subst) = params.get(*name) {
                return Ok(subst.clone());
            }
            type_expr_to_mono_with_env(env, te)
        }
        _ => type_expr_to_mono_with_env(env, te),
    }
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Infers the type of an expression (no type_map recording — for unit tests).
///
/// # Errors
///
/// Returns [`TypeError`] if the expression cannot be typed.
pub fn infer_expr<'a>(env: &mut TypeEnv, expr: &Expr<'a>) -> Result<MonoType, TypeError> {
    let mut sub = Substitution::new();
    let mut map = HashMap::new();
    let ty = infer(env, &mut sub, &mut map, expr)?;
    Ok(sub.apply(&ty))
}

/// Infers and generalizes a top-level declaration, inserting it into `env`
/// and recording sub-expression types in `type_map`.
///
/// # Errors
///
/// Returns [`TypeError`] if the declaration cannot be typed.
pub fn infer_decl_with_map<'a>(
    env: &mut TypeEnv,
    decl: &Decl<'a>,
    type_map: &mut HashMap<Span, MonoType>,
) -> Result<PolyType, TypeError> {
    match decl {
        Decl::Bind {
            name,
            ty: annotation,
            value,
            span,
        } => {
            let mut sub = Substitution::new();
            let inferred = infer(env, &mut sub, type_map, value)?;
            let inferred = sub.apply(&inferred);

            if let Some(ann) = annotation {
                let ann_ty = type_expr_to_mono_with_generics(env, ann)?;
                let ia = sub.apply(&inferred);
                unify(&mut sub, &ia, &ann_ty).map_err(|_| TypeError::AnnotationConflict {
                    annotation: ann_ty.clone(),
                    inferred: ia.clone(),
                    span: *span,
                })?;
                let final_ty = sub.apply(&inferred);
                let poly = generalize(env, &mut sub, &final_ty);
                env.insert(*name, poly.clone());
                type_map.insert(*span, poly.body.clone());
                return Ok(poly);
            }

            let poly = generalize(env, &mut sub, &inferred);
            env.insert(*name, poly.clone());
            type_map.insert(*span, poly.body.clone());
            Ok(poly)
        }

        Decl::TypeAlias { name, params, rhs, .. } => {
            // Create fresh type variables for generic params
            let param_vars: Vec<TypeId> = params.iter().map(|_| env.fresh_var()).collect();
            let param_map: HashMap<&str, MonoType> = params
                .iter()
                .zip(param_vars.iter())
                .map(|(p, &v)| (*p, MonoType::Var(v)))
                .collect();

            match rhs {
                TypeExpr::Sum { variants, .. } => {
                    // Register each variant as a constructor function
                    let mut tag_info = Vec::new();
                    for variant in variants {
                        let payload_tys: Result<Vec<MonoType>, _> = variant
                            .fields
                            .iter()
                            .map(|f| resolve_type_expr_with_env(env, f, &param_map))
                            .collect();
                        let payload_tys = payload_tys?;

                        let ctor_type = if payload_tys.is_empty() {
                            // Nullary constructor: bare tag value
                            PolyType::poly(
                                param_vars.clone(),
                                MonoType::Tag {
                                    name: SmolStr::from(*name),
                                    payload: Rc::from([]),
                                },
                            )
                        } else {
                            // Constructor with payload: (T1, T2, ...) -> TagType
                            PolyType::poly(
                                param_vars.clone(),
                                MonoType::Func {
                                    params: Rc::from(payload_tys.clone()),
                                    ret: Rc::new(MonoType::Tag {
                                        name: SmolStr::from(*name),
                                        payload: Rc::from(payload_tys.clone()),
                                    }),
                                },
                            )
                        };
                        env.insert(variant.name, ctor_type);
                        tag_info.push((SmolStr::from(variant.name), payload_tys));
                    }

                    env.tag_variants
                        .insert(SmolStr::from(*name), tag_info);

                    let poly = PolyType::poly(
                        param_vars,
                        MonoType::Tag {
                            name: SmolStr::from(*name),
                            payload: Rc::from([]),
                        },
                    );
                    env.insert(*name, poly.clone());
                    Ok(poly)
                }
                _ => {
                    // Simple alias: resolve RHS and register
                    let resolved = resolve_type_expr_with_env(env, rhs, &param_map)?;
                    let poly = PolyType::poly(param_vars, resolved);
                    env.insert(*name, poly.clone());
                    Ok(poly)
                }
            }
        }

        Decl::Use { .. } => Ok(PolyType::mono(MonoType::Unit)),
    }
}

/// Infers and generalizes a top-level declaration (no type_map — for unit tests).
///
/// # Errors
///
/// Returns [`TypeError`] if the declaration cannot be typed.
pub fn infer_decl<'a>(env: &mut TypeEnv, decl: &Decl<'a>) -> Result<PolyType, TypeError> {
    let mut map = HashMap::new();
    infer_decl_with_map(env, decl, &mut map)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ast::ast::{Decl, Expr};
    use ast::span::Span;
    use bumpalo::Bump;

    fn sp() -> Span {
        Span::new(0, 1)
    }

    #[test]
    fn infer_i32_literal() {
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        assert_eq!(
            infer_expr(&mut env, Expr::int("42", sp(), &bump)).unwrap(),
            MonoType::I32
        );
    }

    #[test]
    fn infer_bool_literal() {
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        assert_eq!(
            infer_expr(&mut env, Expr::bool(true, sp(), &bump)).unwrap(),
            MonoType::Bool
        );
    }

    #[test]
    fn infer_str_literal() {
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        assert_eq!(
            infer_expr(&mut env, Expr::str("hello", sp(), &bump)).unwrap(),
            MonoType::Str
        );
    }

    #[test]
    fn infer_f64_literal() {
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        assert_eq!(
            infer_expr(&mut env, Expr::float("3.14", sp(), &bump)).unwrap(),
            MonoType::F64
        );
    }

    #[test]
    fn infer_unbound_variable() {
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        assert!(matches!(
            infer_expr(&mut env, Expr::ident("x", sp(), &bump)),
            Err(TypeError::UnboundVariable { .. })
        ));
    }

    #[test]
    fn infer_binary_add_i32() {
        let bump = Bump::new();
        let lhs = Expr::int("1", sp(), &bump);
        let rhs = Expr::int("2", sp(), &bump);
        let expr = Expr::binary(BinOp::Add, lhs, rhs, sp(), &bump);
        let mut env = TypeEnv::new();
        assert_eq!(infer_expr(&mut env, expr).unwrap(), MonoType::I32);
    }

    #[test]
    fn infer_comparison_returns_bool() {
        let bump = Bump::new();
        let lhs = Expr::int("1", sp(), &bump);
        let rhs = Expr::int("2", sp(), &bump);
        let expr = Expr::binary(BinOp::Gt, lhs, rhs, sp(), &bump);
        let mut env = TypeEnv::new();
        assert_eq!(infer_expr(&mut env, expr).unwrap(), MonoType::Bool);
    }

    #[test]
    fn infer_decl_bind_adds_to_env() {
        let bump = Bump::new();
        let val = Expr::int("42", sp(), &bump);
        let decl = Decl::Bind {
            name: "x",
            ty: None,
            value: val,
            span: sp(),
        };
        let mut env = TypeEnv::new();
        let ty = infer_decl(&mut env, &decl).unwrap();
        assert_eq!(ty.body, MonoType::I32);
        assert!(env.contains("x"));
    }

    #[test]
    fn infer_decl_use_stdlib_io() {
        let bump = Bump::new();
        let decl = Decl::Use {
            path: bumpalo::collections::Vec::from_iter_in(["stdlib", "io"], &bump),
            span: sp(),
        };
        let mut env = TypeEnv::new();
        assert!(infer_decl(&mut env, &decl).is_ok());
    }

    #[test]
    fn infer_prelude_id_function() {
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        env.load_prelude();
        let func = Expr::ident("id", sp(), &bump);
        let arg = Expr::int("42", sp(), &bump);
        let args = bumpalo::collections::Vec::from_iter_in([arg], &bump);
        let expr = Expr::app(func, args, sp(), &bump);
        assert_eq!(infer_expr(&mut env, expr).unwrap(), MonoType::I32);
    }

    #[test]
    fn type_map_records_sub_expression_types() {
        // The if-expression span should map to its result type in the map.
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        let cond = Expr::bool(true, Span::new(3, 7), &bump);
        let then_b = Expr::float("1.0", Span::new(10, 13), &bump);
        let else_b = Expr::float("2.0", Span::new(22, 25), &bump);
        let if_span = Span::new(0, 26);
        let expr = bump.alloc(Expr::If {
            condition: cond,
            then_branch: then_b,
            else_branch: else_b,
            span: if_span,
        });
        let mut sub = Substitution::new();
        let mut map = HashMap::new();
        infer(&mut env, &mut sub, &mut map, expr).unwrap();
        assert_eq!(map.get(&if_span), Some(&MonoType::F64));
    }
}
