use std::collections::HashMap;

use crate::error::TypeError;
use crate::types::{MonoType, TypeId};

/// A substitution mapping type variables to resolved types.
#[derive(Debug, Clone, Default)]
pub struct Substitution {
    mappings: HashMap<TypeId, MonoType>,
}

impl Substitution {
    /// Creates an empty substitution.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a mapping from a type variable to a type.
    pub fn insert(&mut self, var: TypeId, ty: MonoType) {
        self.mappings.insert(var, ty);
    }

    /// Looks up a type variable in the substitution.
    #[must_use]
    pub fn lookup(&self, var: TypeId) -> Option<&MonoType> {
        self.mappings.get(&var)
    }

    /// Applies this substitution to a type, resolving all type variables.
    #[must_use]
    pub fn apply(&self, ty: &MonoType) -> MonoType {
        match ty {
            MonoType::Var(id) => {
                if let Some(resolved) = self.lookup(*id) {
                    self.apply(resolved)
                } else {
                    ty.clone()
                }
            }
            MonoType::Array(inner) => MonoType::Array(std::rc::Rc::new(self.apply(inner))),
            MonoType::Func { params, ret } => MonoType::Func {
                params: params.iter().map(|p| self.apply(p)).collect(),
                ret: std::rc::Rc::new(self.apply(ret)),
            },
            MonoType::Record(fields) => MonoType::Record(std::rc::Rc::new(
                fields
                    .iter()
                    .map(|(n, t)| (n.clone(), self.apply(t)))
                    .collect(),
            )),
            MonoType::Tag { name, payload } => MonoType::Tag {
                name: name.clone(),
                payload: payload.iter().map(|t| self.apply(t)).collect(),
            },
            _ => ty.clone(),
        }
    }

    /// Returns the number of mappings in this substitution.
    #[must_use]
    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    /// Returns true if this substitution has no mappings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }
}

/// Unifies two types, producing a substitution that makes them equal.
///
/// # Errors
///
/// Returns [`TypeError::UnificationFailed`] if the types cannot be unified,
/// or [`TypeError::InfiniteType`] if unification would create an infinite type.
pub fn unify(a: &MonoType, b: &MonoType) -> Result<Substitution, TypeError> {
    // TODO: Implement full unification algorithm
    // This is a stub that handles basic cases
    match (a, b) {
        // Same concrete types unify trivially
        _ if a == b => Ok(Substitution::new()),

        // Type variables unify with anything
        (MonoType::Var(id), _) => {
            let mut sub = Substitution::new();
            sub.insert(*id, b.clone());
            Ok(sub)
        }
        (_, MonoType::Var(id)) => {
            let mut sub = Substitution::new();
            sub.insert(*id, a.clone());
            Ok(sub)
        }

        // Arrays unify if their element types unify
        (MonoType::Array(a_inner), MonoType::Array(b_inner)) => unify(a_inner, b_inner),

        // Functions unify if params and return types unify
        (
            MonoType::Func {
                params: ap,
                ret: ar,
            },
            MonoType::Func {
                params: bp,
                ret: br,
            },
        ) => {
            if ap.len() != bp.len() {
                return Err(TypeError::ArityMismatch {
                    expected: ap.len(),
                    got: bp.len(),
                    span: ast::span::Span::new(0, 0), // TODO: proper span
                });
            }
            let mut sub = Substitution::new();
            for (p, q) in ap.iter().zip(bp.iter()) {
                let s = unify(p, q)?;
                merge_substitution(&mut sub, &s)?;
            }
            let s = unify(ar, br)?;
            merge_substitution(&mut sub, &s)?;
            Ok(sub)
        }

        // Mismatched concrete types
        _ => Err(TypeError::UnificationFailed {
            expected: a.clone(),
            got: b.clone(),
            span: ast::span::Span::new(0, 0), // TODO: proper span
        }),
    }
}

fn merge_substitution(base: &mut Substitution, new: &Substitution) -> Result<(), TypeError> {
    for ty in base.mappings.values_mut() {
        *ty = new.apply(ty);
    }
    for (id, ty) in &new.mappings {
        if !base.mappings.contains_key(id) {
            base.insert(*id, ty.clone());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unify_same_concrete_type() {
        let sub = unify(&MonoType::I32, &MonoType::I32).unwrap();
        assert!(sub.is_empty());
    }

    #[test]
    fn unify_var_with_concrete() {
        let sub = unify(&MonoType::Var(TypeId(0)), &MonoType::I32).unwrap();
        assert_eq!(sub.lookup(TypeId(0)), Some(&MonoType::I32));
    }

    #[test]
    fn unify_concrete_with_var() {
        let sub = unify(&MonoType::Str, &MonoType::Var(TypeId(0))).unwrap();
        assert_eq!(sub.lookup(TypeId(0)), Some(&MonoType::Str));
    }

    #[test]
    fn unify_mismatched_concrete_types() {
        let err = unify(&MonoType::I32, &MonoType::Str).unwrap_err();
        assert!(matches!(err, TypeError::UnificationFailed { .. }));
    }

    #[test]
    fn unify_arrays() {
        let sub = unify(
            &MonoType::Array(std::rc::Rc::new(MonoType::I32)),
            &MonoType::Array(std::rc::Rc::new(MonoType::Var(TypeId(0)))),
        )
        .unwrap();
        assert_eq!(sub.lookup(TypeId(0)), Some(&MonoType::I32));
    }

    #[test]
    fn unify_functions() {
        let a = MonoType::Func {
            params: std::rc::Rc::from([MonoType::I32]),
            ret: std::rc::Rc::new(MonoType::Bool),
        };
        let b = MonoType::Func {
            params: std::rc::Rc::from([MonoType::Var(TypeId(0))]),
            ret: std::rc::Rc::new(MonoType::Var(TypeId(1))),
        };
        let sub = unify(&a, &b).unwrap();
        assert_eq!(sub.lookup(TypeId(0)), Some(&MonoType::I32));
        assert_eq!(sub.lookup(TypeId(1)), Some(&MonoType::Bool));
    }

    #[test]
    fn unify_arity_mismatch() {
        let a = MonoType::Func {
            params: std::rc::Rc::from([MonoType::I32]),
            ret: std::rc::Rc::new(MonoType::Bool),
        };
        let b = MonoType::Func {
            params: std::rc::Rc::from([MonoType::I32, MonoType::Str]),
            ret: std::rc::Rc::new(MonoType::Bool),
        };
        let err = unify(&a, &b).unwrap_err();
        assert!(matches!(err, TypeError::ArityMismatch { .. }));
    }

    #[test]
    fn substitution_apply_resolves_chain() {
        let mut sub = Substitution::new();
        sub.insert(TypeId(0), MonoType::Var(TypeId(1)));
        sub.insert(TypeId(1), MonoType::I32);
        let resolved = sub.apply(&MonoType::Var(TypeId(0)));
        assert_eq!(resolved, MonoType::I32);
    }
}
