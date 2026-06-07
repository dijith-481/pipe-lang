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
        Expr::IntLiteral(_, _) => Ok(MonoType::I32),
        Expr::FloatLiteral(_, _) => Ok(MonoType::F64),
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
            let then_ty = infer_expr(_env, then_branch)?;
            let _else_ty = infer_expr(_env, else_branch)?;
            // TODO: unify then_ty and else_ty
            Ok(then_ty)
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
pub fn infer_decl<'a>(env: &mut TypeEnv, decl: &ast::ast::Decl<'a>) -> Result<PolyType, TypeError> {
    match decl {
        ast::ast::Decl::Bind {
            name,
            ty: _,
            value,
            span: _,
        } => {
            let ty = infer_expr(env, value)?;
            let poly = PolyType::mono(ty);
            env.insert(*name, poly.clone());
            Ok(poly)
        }
        ast::ast::Decl::TypeAlias {
            name,
            params: _,
            rhs: _,
            span: _,
        } => {
            // TODO: Register type alias
            // For now, return unit
            let poly = PolyType::mono(MonoType::Unit);
            env.insert(*name, poly.clone());
            Ok(poly)
        }
        ast::ast::Decl::Use { path, span: _ } => {
            // Handle standard library imports
            let path_str = path.join("::");
            match path_str.as_str() {
                "stdlib::io" => {
                    // IO module types would be loaded here
                    // For now, this is a no-op (IO builtins are registered at runtime)
                    Ok(PolyType::mono(MonoType::Unit))
                }
                "stdlib::list" => {
                    // List module types would be loaded here
                    Ok(PolyType::mono(MonoType::Unit))
                }
                "stdlib::option" => {
                    // Option module is already in prelude
                    Ok(PolyType::mono(MonoType::Unit))
                }
                "stdlib::result" => {
                    // Result module is already in prelude
                    Ok(PolyType::mono(MonoType::Unit))
                }
                _ => {
                    // Unknown module — for now, just return unit
                    // In a full implementation, this would look up the module
                    Ok(PolyType::mono(MonoType::Unit))
                }
            }
        }
    }
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
        let expr = Expr::int("42", Span::new(0, 2), &bump);
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
        let expr = Expr::float("3.14", Span::new(0, 4), &bump);
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
        let lhs = Expr::int("1", Span::new(0, 1), &bump);
        let rhs = Expr::int("2", Span::new(4, 5), &bump);
        let expr = Expr::binary(BinOp::Add, lhs, rhs, Span::new(0, 5), &bump);
        let mut env = TypeEnv::new();
        let ty = infer_expr(&mut env, expr).unwrap();
        assert_eq!(ty, MonoType::I32);
    }

    #[test]
    fn infer_comparison_returns_bool() {
        let bump = Bump::new();
        let lhs = Expr::int("1", Span::new(0, 1), &bump);
        let rhs = Expr::int("2", Span::new(4, 5), &bump);
        let expr = Expr::binary(BinOp::Gt, lhs, rhs, Span::new(0, 5), &bump);
        let mut env = TypeEnv::new();
        let ty = infer_expr(&mut env, expr).unwrap();
        assert_eq!(ty, MonoType::Bool);
    }

    #[test]
    fn infer_decl_bind_adds_to_env() {
        let bump = Bump::new();
        let val = Expr::int("42", Span::new(8, 10), &bump);
        let decl = ast::ast::Decl::Bind {
            name: "x",
            ty: None,
            value: val,
            span: Span::new(0, 10),
        };
        let mut env = TypeEnv::new();
        let ty = infer_decl(&mut env, &decl).unwrap();
        assert_eq!(ty, PolyType::mono(MonoType::I32));
        assert!(env.contains("x"));
    }

    #[test]
    fn infer_decl_use_stdlib_io() {
        let bump = Bump::new();
        let decl = ast::ast::Decl::Use {
            path: bumpalo::collections::Vec::from_iter_in(["stdlib", "io"], &bump),
            span: Span::new(0, 13),
        };
        let mut env = TypeEnv::new();
        let result = infer_decl(&mut env, &decl);
        assert!(result.is_ok());
    }

    #[test]
    fn infer_decl_use_unknown_module() {
        let bump = Bump::new();
        let decl = ast::ast::Decl::Use {
            path: bumpalo::collections::Vec::from_iter_in(["stdlib", "nonexistent"], &bump),
            span: Span::new(0, 20),
        };
        let mut env = TypeEnv::new();
        let result = infer_decl(&mut env, &decl);
        assert!(result.is_ok()); // Unknown modules return unit for now
    }

    #[test]
    fn infer_prelude_id_function() {
        let bump = Bump::new();
        let mut env = TypeEnv::new();
        env.load_prelude();

        // id(42) should return i32
        let func = Expr::ident("id", Span::new(0, 2), &bump);
        let arg = Expr::int("42", Span::new(3, 5), &bump);
        let args = bumpalo::collections::Vec::from_iter_in([arg], &bump);
        let expr = Expr::app(func, args, Span::new(0, 6), &bump);
        let ty = infer_expr(&mut env, expr).unwrap();
        assert_eq!(ty, MonoType::I32);
    }
}
