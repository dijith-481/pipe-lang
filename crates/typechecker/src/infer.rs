use ast::ast::{BinOp, Expr};

use crate::env::TypeEnv;
use crate::error::TypeError;
use crate::types::{MonoType, PolyType};

/// Infers the type of an expression.
///
/// # Errors
///
/// Returns [`TypeError`] if the expression cannot be typed.
pub fn infer_expr<'a>(_env: &mut TypeEnv, expr: &Expr<'a>) -> Result<MonoType, TypeError> {
    // TODO: Implement full type inference
    // This is a stub that handles basic cases
    match expr {
        Expr::I8(_, _) => Ok(MonoType::I8),
        Expr::I16(_, _) => Ok(MonoType::I16),
        Expr::I32(_, _) => Ok(MonoType::I32),
        Expr::I64(_, _) => Ok(MonoType::I64),
        Expr::U8(_, _) => Ok(MonoType::U8),
        Expr::U16(_, _) => Ok(MonoType::U16),
        Expr::U32(_, _) => Ok(MonoType::U32),
        Expr::U64(_, _) => Ok(MonoType::U64),
        Expr::Usize(_, _) => Ok(MonoType::Usize),
        Expr::F32(_, _) => Ok(MonoType::F32),
        Expr::F64(_, _) => Ok(MonoType::F64),
        Expr::Bool(_, _) => Ok(MonoType::Bool),
        Expr::Str(_, _) => Ok(MonoType::Str),

        Expr::Ident(name, span) => {
            Err(TypeError::UnboundVariable {
                name: name.to_string(),
                span: *span,
            })
            // TODO: look up in env
        }

        Expr::Binary {
            op, left, right, ..
        } => {
            let l = infer_expr(_env, left)?;
            let _r = infer_expr(_env, right)?;
            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                    // TODO: check both operands are numeric and same type
                    Ok(l)
                }
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    Ok(MonoType::Bool)
                }
                BinOp::And | BinOp::Or => {
                    // TODO: check both operands are Bool
                    Ok(MonoType::Bool)
                }
            }
        }

        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let _cond = infer_expr(_env, condition)?;
            // TODO: check cond is Bool
            let then_ty = infer_expr(_env, then_branch)?;
            if let Some(else_expr) = else_branch {
                let _else_ty = infer_expr(_env, else_expr)?;
                // TODO: unify then_ty and else_ty
                Ok(then_ty)
            } else {
                // TODO: if without else should be Unit
                Ok(MonoType::Unit)
            }
        }

        _ => {
            // TODO: implement inference for all other expression variants
            Ok(MonoType::I32) // placeholder
        }
    }
}

/// Infers the type of a top-level declaration.
///
/// # Errors
///
/// Returns [`TypeError`] if the declaration cannot be typed.
pub fn infer_decl<'a>(
    _env: &mut TypeEnv,
    _decl: &ast::ast::Decl<'a>,
) -> Result<PolyType, TypeError> {
    // TODO: Implement declaration inference
    // This is a stub
    Ok(PolyType::mono(MonoType::Unit))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ast::span::Span;
    use bumpalo::Bump;

    #[test]
    fn infer_i32_literal() {
        let bump = Bump::new();
        let expr = Expr::i32(42, Span::new(0, 2), &bump);
        let mut env = TypeEnv::new();
        let ty = infer_expr(&mut env, expr).unwrap();
        assert_eq!(ty, MonoType::I32);
    }

    #[test]
    fn infer_bool_literal() {
        let bump = Bump::new();
        let expr = Expr::bool(true, Span::new(0, 4), &bump);
        let mut env = TypeEnv::new();
        let ty = infer_expr(&mut env, expr).unwrap();
        assert_eq!(ty, MonoType::Bool);
    }

    #[test]
    fn infer_str_literal() {
        let bump = Bump::new();
        let expr = Expr::str("hello", Span::new(0, 7), &bump);
        let mut env = TypeEnv::new();
        let ty = infer_expr(&mut env, expr).unwrap();
        assert_eq!(ty, MonoType::Str);
    }

    #[test]
    fn infer_f64_literal() {
        let bump = Bump::new();
        let expr = Expr::f64(3.14, Span::new(0, 4), &bump);
        let mut env = TypeEnv::new();
        let ty = infer_expr(&mut env, expr).unwrap();
        assert_eq!(ty, MonoType::F64);
    }

    #[test]
    fn infer_unbound_variable() {
        let bump = Bump::new();
        let expr = Expr::ident("x", Span::new(0, 1), &bump);
        let mut env = TypeEnv::new();
        let err = infer_expr(&mut env, expr).unwrap_err();
        assert!(matches!(err, TypeError::UnboundVariable { .. }));
    }

    #[test]
    fn infer_binary_add_i32() {
        let bump = Bump::new();
        let lhs = Expr::i32(1, Span::new(0, 1), &bump);
        let rhs = Expr::i32(2, Span::new(4, 5), &bump);
        let expr = Expr::binary(BinOp::Add, lhs, rhs, Span::new(0, 5), &bump);
        let mut env = TypeEnv::new();
        let ty = infer_expr(&mut env, expr).unwrap();
        assert_eq!(ty, MonoType::I32);
    }

    #[test]
    fn infer_comparison_returns_bool() {
        let bump = Bump::new();
        let lhs = Expr::i32(1, Span::new(0, 1), &bump);
        let rhs = Expr::i32(2, Span::new(4, 5), &bump);
        let expr = Expr::binary(BinOp::Gt, lhs, rhs, Span::new(0, 5), &bump);
        let mut env = TypeEnv::new();
        let ty = infer_expr(&mut env, expr).unwrap();
        assert_eq!(ty, MonoType::Bool);
    }
}
