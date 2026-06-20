pub mod env;
pub mod error;
pub mod infer;
pub mod types;
pub mod unify;

pub use crate::env::{TagVariants, TypeEnv};
pub use crate::error::TypeError;
pub use crate::infer::{
    infer_decl, infer_expr, type_expr_to_mono_with_env, type_expr_to_mono_with_generics,
};
pub use crate::types::{MonoType, PolyType, TypeId};
pub use crate::unify::{Substitution, unify};

use ast::ast::{Decl, Expr, Program};
use ast::span::Span;
use std::collections::HashMap;
use std::rc::Rc;

/// The output of a successful typecheck pass.
pub struct TypedProgram<'a> {
    pub ast: &'a Program<'a>,
    pub env: TypeEnv,
    /// Maps every expression span (and decl span) to its fully-resolved type.
    pub type_map: HashMap<Span, MonoType>,
    /// Maps tag type names (e.g. "Option", "Result") to their variant info.
    /// Populated from the prelude and user-defined type declarations.
    pub tag_variants: TagVariants,
}

/// Typechecks a parsed program, returning a [`TypedProgram`] with a complete
/// span→type map for the IR lowerer and LSP hover.
///
/// # Errors
///
/// Returns `Vec<TypeError>` if any declaration fails to typecheck.
pub fn typecheck<'a>(ast: &'a Program<'a>) -> Result<TypedProgram<'a>, Vec<TypeError>> {
    let mut env = TypeEnv::new();
    env.load_prelude();

    // Forward-declare all top-level functions so recursive calls work.
    // For each `let f = (params) => body`, insert `f` into the env with a
    // partial function type built from parameter annotations (or fresh vars).
    // The main inference pass then resolves everything through unification.
    forward_declare_top_level(&mut env, ast);

    let mut errors = Vec::new();
    let mut type_map = HashMap::new();

    for decl in &ast.decls {
        if let Err(e) = infer::infer_decl_with_map(&mut env, decl, &mut type_map) {
            errors.push(e);
        }
    }

    if errors.is_empty() {
        let tag_variants = env.tag_variants.clone();
        Ok(TypedProgram {
            ast,
            env,
            type_map,
            tag_variants,
        })
    } else {
        Err(errors)
    }
}

/// Pre-scans top-level declarations and inserts function signatures into the
/// environment so that recursive calls resolve during inference.
fn forward_declare_top_level<'a>(env: &mut TypeEnv, ast: &Program<'a>) {
    for decl in &ast.decls {
        if let Decl::Bind {
            name,
            value,
            ty: annotation,
            ..
        } = decl
        {
            // If there's a type annotation, use it directly.
            if let Some(ann) = annotation
                && let Ok(mono) = infer::type_expr_to_mono_with_generics(env, ann)
            {
                env.insert(*name, PolyType::mono(mono));
                continue;
            }

            // For lambdas, build a partial function type from param annotations.
            if let Expr::Lambda { params, .. } = value {
                let param_tys: Vec<MonoType> = params
                    .iter()
                    .map(|p| {
                        if let Some(ann) = p.ty {
                            infer::type_expr_to_mono_with_env(env, ann)
                                .unwrap_or(MonoType::Var(env.fresh_var()))
                        } else {
                            MonoType::Var(env.fresh_var())
                        }
                    })
                    .collect();
                let ret_var = MonoType::Var(env.fresh_var());
                let func_ty = MonoType::Func {
                    params: Rc::from(param_tys.as_slice()),
                    ret: Rc::new(ret_var),
                };
                env.insert(*name, PolyType::mono(func_ty));
            }
        }
    }
}
