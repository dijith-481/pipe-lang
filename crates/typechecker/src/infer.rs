use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use ast::SmolStr;
use ast::ast::NodeId;
use ast::ast::{BinOp, Decl, Expr, LiteralPattern, MatchArm, Pattern, Stmt, TypeExpr, UnaryOp};
use ast::span::Span;

use crate::env::TypeEnv;
use crate::error::TypeError;
use crate::exhaustiveness::check_exhaustive;
use crate::types::{MonoType, PolyType, TypeId};
use crate::unify::{Substitution, unify};

// ---------------------------------------------------------------------------
// Free type variables
// ---------------------------------------------------------------------------

fn free_vars_mono(ty: &MonoType, out: &mut HashSet<TypeId>) {
    match ty {
        // Both unconstrained and constrained vars are free until resolved.
        MonoType::Var(id) | MonoType::IntVar(id) | MonoType::FloatVar(id) => {
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
        // Preserve the constraint kind (Int/Float) when mapping quantified variables.
        MonoType::IntVar(id) => m.get(id).cloned().unwrap_or_else(|| ty.clone()),
        MonoType::FloatVar(id) => m.get(id).cloned().unwrap_or_else(|| ty.clone()),
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
                MonoType::Tag { name, payload } if name.as_str() == "Tuple" => payload.to_vec(),
                _ => vec![from_resolved],
            };
            Ok(MonoType::Func {
                params: Rc::from(params),
                ret: Rc::new(type_expr_to_mono_inner(env, to, generics)?),
            })
        }
        TypeExpr::Tuple { types, span: _ } => {
            let payload: Result<Vec<MonoType>, _> = types
                .iter()
                .map(|t| type_expr_to_mono_inner(env, t, generics))
                .collect();
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
pub fn type_expr_to_mono_with_env(env: &TypeEnv, te: &TypeExpr<'_>) -> Result<MonoType, TypeError> {
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
        Pattern::Wildcard(_, _) => {
            let v = env.fresh_var();
            sub.ensure_key(v);
            Ok(MonoType::Var(v))
        }
        Pattern::Binding(_, name, _) => {
            let v = env.fresh_var();
            sub.ensure_key(v);
            let ty = MonoType::Var(v);
            env.insert(*name, PolyType::mono(ty.clone()));
            Ok(ty)
        }
        Pattern::Literal(_, lit, _span) => match lit {
            LiteralPattern::Int(text) => {
                // Use a constrained integer variable so the pattern type unifies
                // with the subject — if the subject is u8, the pattern becomes u8.
                let resolved = int_literal_type(text);
                if matches!(resolved, MonoType::I32) && !text.ends_with("i32") {
                    // Un-suffixed integer literal in a pattern: emit IntVar.
                    let id = env.fresh_var();
                    sub.ensure_key(id);
                    Ok(MonoType::IntVar(id))
                } else {
                    Ok(resolved)
                }
            }
            LiteralPattern::Float(text) => {
                let resolved = float_literal_type(text);
                if matches!(resolved, MonoType::F64) && !text.ends_with("f64") {
                    let id = env.fresh_var();
                    sub.ensure_key(id);
                    Ok(MonoType::FloatVar(id))
                } else {
                    Ok(resolved)
                }
            }
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
    MonoType::I32
}

fn check_int_overflow(text: &str, ty: &MonoType, span: Span) -> Result<(), TypeError> {
    let clean = text.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let val: i128 = match clean.parse() {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    let in_range = match ty {
        MonoType::I8 => val >= i128::from(i8::MIN) && val <= i128::from(i8::MAX),
        MonoType::I16 => val >= i128::from(i16::MIN) && val <= i128::from(i16::MAX),
        MonoType::I32 => val >= i128::from(i32::MIN) && val <= i128::from(i32::MAX),
        MonoType::I64 => val >= i128::from(i64::MIN) && val <= i128::from(i64::MAX),
        MonoType::U8 => val >= 0 && val <= i128::from(u8::MAX),
        MonoType::U16 => val >= 0 && val <= i128::from(u16::MAX),
        MonoType::U32 => val >= 0 && val <= i128::from(u32::MAX),
        MonoType::U64 => val >= 0 && val <= i128::from(u64::MAX),
        MonoType::Usize => val >= 0 && val <= i128::from(usize::MAX as u64),
        _ => return Ok(()),
    };
    if in_range {
        Ok(())
    } else {
        Err(TypeError::NumericOverflow {
            ty: ty.clone(),
            span,
        })
    }
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
/// Every expression's fully-resolved id→type is inserted into `type_map`
/// before returning, enabling the IR lowerer to look up any sub-expression's
/// type by [`NodeId`] without a second traversal.
///
/// # Errors
///
/// Returns [`TypeError`] on type mismatches, unbound variables, or infinite types.
pub fn infer<'a>(
    env: &mut TypeEnv,
    sub: &mut Substitution,
    type_map: &mut HashMap<NodeId, MonoType>,
    expr: &Expr<'a>,
) -> Result<MonoType, TypeError> {
    let result = infer_inner(env, sub, type_map, expr)?;
    let resolved = sub.apply(&result);
    type_map.insert(expr.id(), resolved.clone());
    Ok(resolved)
}

fn infer_inner<'a>(
    env: &mut TypeEnv,
    sub: &mut Substitution,
    type_map: &mut HashMap<NodeId, MonoType>,
    expr: &Expr<'a>,
) -> Result<MonoType, TypeError> {
    match expr {
        Expr::IntLiteral(_, text, span) => {
            let resolved = int_literal_type(text);
            if matches!(resolved, MonoType::I32) && !text.ends_with("i32") {
                // Un-suffixed integer literal: emit a constrained IntVar so the
                // type can be narrowed by an annotation (e.g. `let x: u8 = 42`).
                check_int_overflow(text, &MonoType::I32, *span)?;
                let id = env.fresh_var();
                sub.ensure_key(id);
                Ok(MonoType::IntVar(id))
            } else {
                check_int_overflow(text, &resolved, *span)?;
                Ok(resolved)
            }
        }
        Expr::FloatLiteral(_, text, _) => {
            let resolved = float_literal_type(text);
            if matches!(resolved, MonoType::F64) && !text.ends_with("f64") {
                // Un-suffixed float literal: emit FloatVar so annotations can narrow it.
                let id = env.fresh_var();
                sub.ensure_key(id);
                Ok(MonoType::FloatVar(id))
            } else {
                Ok(resolved)
            }
        }
        Expr::Bool(_, _, _) => Ok(MonoType::Bool),
        Expr::Str(_, _, _) => Ok(MonoType::Str),

        Expr::Ident(_, name, span) => {
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

        Expr::Application {
            func, args, span, ..
        } => {
            let mut func_ty = infer(env, sub, type_map, func)?;
            let arg_tys: Vec<MonoType> = args
                .iter()
                .map(|a| infer(env, sub, type_map, a))
                .collect::<Result<_, TypeError>>()?;

            // Method dispatch: if the function is a bare name and the first arg
            // is a Tag type, try the qualified method name (e.g. Option.map).
            if let Expr::Ident(_, name, _) = *func
                && !args.is_empty()
            {
                let first_arg_ty = sub.apply(&arg_tys[0]);

                // Extract type name for both Tags AND Primitives
                let type_namespace = match &first_arg_ty {
                    MonoType::Tag { name: tag_name, .. } => Some(tag_name.as_str()),
                    MonoType::I8 => Some("I8"),
                    MonoType::I16 => Some("I16"),
                    MonoType::I32 => Some("I32"),
                    MonoType::I64 => Some("I64"),
                    MonoType::U8 => Some("U8"),
                    MonoType::U16 => Some("U16"),
                    MonoType::U32 => Some("U32"),
                    MonoType::U64 => Some("U64"),
                    MonoType::Usize => Some("Usize"),
                    MonoType::F32 => Some("F32"),
                    MonoType::F64 => Some("F64"),
                    MonoType::Str => Some("Str"),
                    MonoType::Bool => Some("Bool"),
                    MonoType::Array(_) => Some("Array"),
                    _ => None,
                };

                if let Some(ns) = type_namespace {
                    let qualified = format!("{}.{}", ns, name);
                    if let Some(qualified_poly) = env.lookup(&qualified).cloned() {
                        func_ty = instantiate(env, sub, &qualified_poly);
                    }
                }
            }

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
            // Re-resolve argument types after unification.
            // Lambda argument types are stored in the type_map before unification
            // happens, so their parameter types may still be unresolved type
            // variables.  Re-resolve and re-insert now that the function
            // signature has been unified.
            for arg in args {
                if let Some(ty) = type_map.get(&arg.id()) {
                    type_map.insert(arg.id(), sub.apply(ty));
                }
            }
            Ok(sub.apply(&ret_ty))
        }

        Expr::Binary {
            op,
            left,
            right,
            span,
            ..
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

        Expr::Unary {
            op, operand, span, ..
        } => {
            let ty = infer(env, sub, type_map, operand)?;
            let ta = sub.apply(&ty);
            match op {
                UnaryOp::Neg => {
                    if !ta.is_numeric() && !matches!(ta, MonoType::Var(_)) {
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
            ..
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
            ..
        } => {
            if arms.is_empty() {
                return Err(TypeError::NonExhaustiveMatch { span: *span });
            }
            let subj_ty = infer(env, sub, type_map, subject)?;
            let mut result_ty: Option<MonoType> = None;
            for arm in arms {
                infer_arm(env, sub, type_map, arm, &subj_ty, span, &mut result_ty)?;
            }
            let subj_applied = sub.apply(&subj_ty);
            check_exhaustive(&env.tag_variants, &subj_applied, arms, *span)?;
            Ok(sub.apply(result_ty.as_ref().unwrap()))
        }

        Expr::Array { elems, span, .. } => {
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
            ..
        } => {
            let raw_obj_ty = infer(env, sub, type_map, object)?;
            let oa = sub.apply(&raw_obj_ty);
            match &oa {
                MonoType::Record(fields) => {
                    if let Some(ty) = fields.get(*field).cloned() {
                        Ok(ty)
                    } else if matches!(&raw_obj_ty, MonoType::Var(_)) {
                        // Row-polymorphic record extension: the variable was
                        // previously bound to a record that doesn't have this
                        // field yet.  Extend the record and unify.
                        let fv = env.fresh_var();
                        sub.ensure_key(fv);
                        let fv_ty = MonoType::Var(fv);
                        let mut extended = (**fields).clone();
                        extended.insert((*field).into(), fv_ty.clone());
                        let expected_rec = MonoType::Record(Rc::new(extended));
                        unify(sub, &oa, &expected_rec).map_err(|_| TypeError::FieldNotFound {
                            field: (*field).to_string(),
                            span: *span,
                        })?;
                        Ok(sub.apply(&fv_ty))
                    } else {
                        Err(TypeError::FieldNotFound {
                            field: (*field).to_string(),
                            span: *span,
                        })
                    }
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

        Expr::Index {
            array, index, span, ..
        } => {
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
    type_map: &mut HashMap<NodeId, MonoType>,
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
        Pattern::Binding(_, name, _) => env.insert(*name, poly),
        Pattern::Wildcard(_, _) => {}
        _ => {
            bind_pattern(env, sub, pat)?;
        }
    }
    Ok(())
}

fn infer_arm<'a>(
    env: &mut TypeEnv,
    sub: &mut Substitution,
    type_map: &mut HashMap<NodeId, MonoType>,
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
    type_map: &mut HashMap<NodeId, MonoType>,
) -> Result<PolyType, TypeError> {
    match decl {
        Decl::Bind {
            name,
            ty: annotation,
            value,
            span,
            id: decl_id,
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
                // Refresh all type_map entries: resolve type variables that
                // were bound after the entry was first recorded.
                for (_, ty) in type_map.iter_mut() {
                    *ty = sub.apply(ty);
                }
                let poly = generalize(env, &mut sub, &final_ty);
                env.insert(*name, poly.clone());
                type_map.insert(*decl_id, poly.body.clone());
                return Ok(poly);
            }

            // Refresh all type_map entries.
            for (_, ty) in type_map.iter_mut() {
                *ty = sub.apply(ty);
            }
            let poly = generalize(env, &mut sub, &inferred);
            env.insert(*name, poly.clone());
            type_map.insert(*decl_id, poly.body.clone());
            Ok(poly)
        }

        Decl::TypeAlias {
            name, params, rhs, ..
        } => {
            // Create fresh type variables for generic params
            let param_vars: Vec<TypeId> = params.iter().map(|_| env.fresh_var()).collect();
            let param_map: HashMap<&str, MonoType> = params
                .iter()
                .zip(param_vars.iter())
                .map(|(p, &v)| (*p, MonoType::Var(v)))
                .collect();

            match rhs {
                TypeExpr::Sum { variants, .. } => {
                    // Pass 1: Resolve variant payload types to build the combined payloads list.
                    // We first register a placeholder tag type in env so recursive references resolve.
                    let placeholder_poly = PolyType::poly(
                        param_vars.clone(),
                        MonoType::Tag {
                            name: SmolStr::from(*name),
                            payload: Rc::from([]),
                        },
                    );
                    env.insert(*name, placeholder_poly);

                    let mut tag_info = Vec::new();
                    for variant in variants {
                        let payload_tys: Result<Vec<MonoType>, _> = variant
                            .fields
                            .iter()
                            .map(|f| resolve_type_expr_with_env(env, f, &param_map))
                            .collect();
                        let payload_tys = payload_tys?;
                        tag_info.push((SmolStr::from(variant.name), payload_tys));
                    }

                    let combined: Vec<MonoType> =
                        tag_info.iter().flat_map(|(_, ptys)| ptys.clone()).collect();
                    env.tag_variants.insert(SmolStr::from(*name), tag_info.clone());

                    // Register the final tag type in env so constructors resolve to the full tag type.
                    let poly = PolyType::poly(
                        param_vars.clone(),
                        MonoType::Tag {
                            name: SmolStr::from(*name),
                            payload: Rc::from(combined.clone()),
                        },
                    );
                    env.insert(*name, poly.clone());

                    // Pass 2: Register each variant constructor function returning the full tag type.
                    for (variant_name, payload_tys) in tag_info {
                        let ctor_type = if payload_tys.is_empty() {
                            PolyType::poly(
                                param_vars.clone(),
                                MonoType::Tag {
                                    name: SmolStr::from(*name),
                                    payload: Rc::from(combined.clone()),
                                },
                            )
                        } else {
                            PolyType::poly(
                                param_vars.clone(),
                                MonoType::Func {
                                    params: Rc::from(payload_tys.clone()),
                                    ret: Rc::new(MonoType::Tag {
                                        name: SmolStr::from(*name),
                                        payload: Rc::from(combined.clone()),
                                    }),
                                },
                            )
                        };
                        env.insert(variant_name.as_str(), ctor_type);
                    }

                    type_map.insert(decl.id(), poly.body.clone());
                    Ok(poly)
                }
                _ => {
                    // Simple alias: resolve RHS and register
                    let resolved = resolve_type_expr_with_env(env, rhs, &param_map)?;
                    let poly = PolyType::poly(param_vars, resolved);
                    env.insert(*name, poly.clone());
                    type_map.insert(decl.id(), poly.body.clone());
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
    use ast::ast::{Decl, Expr, NodeId, TypeExpr, TypeVariant};
    use ast::span::Span;
    use bumpalo::Bump;

    fn sp() -> Span {
        Span::new(0, 1)
    }

    #[test]
    fn infer_i32_literal() {
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        // Un-suffixed int defaults to I32.
        assert_eq!(
            infer_expr(&mut env, Expr::int("42", sp(), &bump)).unwrap(),
            MonoType::I32
        );
    }

    #[test]
    fn infer_suffixed_i64_literal() {
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        assert_eq!(
            infer_expr(&mut env, Expr::int("42i64", sp(), &bump)).unwrap(),
            MonoType::I64
        );
    }

    #[test]
    fn infer_suffixed_u8_literal() {
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        assert_eq!(
            infer_expr(&mut env, Expr::int("255u8", sp(), &bump)).unwrap(),
            MonoType::U8
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
        // Un-suffixed float defaults to F64.
        assert_eq!(
            infer_expr(&mut env, Expr::float("3.14", sp(), &bump)).unwrap(),
            MonoType::F64
        );
    }

    #[test]
    fn infer_suffixed_f32_literal() {
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        assert_eq!(
            infer_expr(&mut env, Expr::float("3.14f32", sp(), &bump)).unwrap(),
            MonoType::F32
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
            id: NodeId(0),
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
            id: NodeId(0),
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
        // The if-expression id should map to its result type in the map.
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        let cond = bump.alloc(Expr::Bool(NodeId(0), true, Span::new(3, 7)));
        let then_b = bump.alloc(Expr::FloatLiteral(NodeId(1), "1.0", Span::new(10, 13)));
        let else_b = bump.alloc(Expr::FloatLiteral(NodeId(2), "2.0", Span::new(22, 25)));
        let if_id = NodeId(3);
        let expr = bump.alloc(Expr::If {
            id: if_id,
            condition: cond,
            then_branch: then_b,
            else_branch: else_b,
            span: Span::new(0, 26),
        });
        let mut sub = Substitution::new();
        let mut map = HashMap::new();
        infer(&mut env, &mut sub, &mut map, expr).unwrap();
        assert_eq!(map.get(&if_id), Some(&MonoType::F64));
    }

    #[test]
    fn intvar_defaults_to_i32() {
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        // `42` with no annotation should default to I32.
        let expr = Expr::int("42", sp(), &bump);
        assert_eq!(infer_expr(&mut env, expr).unwrap(), MonoType::I32);
    }

    #[test]
    fn floatvar_defaults_to_f64() {
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        let expr = Expr::float("3.14", sp(), &bump);
        assert_eq!(infer_expr(&mut env, expr).unwrap(), MonoType::F64);
    }

    // -----------------------------------------------------------------------
    // Self-referential (recursive) tag type aliases fail to typecheck.
    //
    // `type Expr = | Num(f64) | Add(Expr, Expr) | Neg(Expr)` defines a
    // recursive sum type.  The typechecker at infer.rs:1098 resolves
    // variant payload types BEFORE the type name is registered in the
    // env (line 1139).  When a payload references the type itself (e.g.
    // `Add(Expr, Expr)`), the lookup returns "unbound variable".
    //
    // This prevents expression-evaluator.pp and similar recursive ADTs
    // from typechecking.
    // -----------------------------------------------------------------------

    #[test]
    fn recursive_tag_type_alias_fails_to_typecheck() {
        let bump = Bump::new();

        // Build `type Expr = | Num(f64) | Neg(Expr)`
        let num_variant = TypeVariant {
            name: "Num",
            fields: bumpalo::collections::Vec::from_iter_in([TypeExpr::Named("f64", sp())], &bump),
            span: sp(),
        };
        let neg_variant = TypeVariant {
            name: "Neg",
            fields: bumpalo::collections::Vec::from_iter_in([TypeExpr::Named("Expr", sp())], &bump),
            span: sp(),
        };
        let sum_ty = TypeExpr::Sum {
            variants: bumpalo::collections::Vec::from_iter_in([num_variant, neg_variant], &bump),
            span: sp(),
        };

        let decl = Decl::TypeAlias {
            id: NodeId(0),
            name: "Expr",
            params: bumpalo::collections::Vec::new_in(&bump),
            rhs: &sum_ty,
            span: sp(),
        };

        let mut env = TypeEnv::new();
        let result = infer_decl(&mut env, &decl);

        // The type alias SHOULD typecheck successfully, registering the
        // recursive tag type in the env.  But infer.rs:1098 resolves
        // variant fields before the name is inserted (line 1139), so
        // `Neg(Expr)` fails with "UnboundVariable: Expr".
        assert!(
            result.is_ok(),
            "Recursive type alias `type Expr = | Num(f64) | Neg(Expr)` \
             should typecheck but got: {:?}. \
             Bug: infer.rs:1098 resolves variant payloads before the \
             type name is registered at line 1139, so self-referential \
             tags fail with UnboundVariable.",
            result,
        );
    }
}
