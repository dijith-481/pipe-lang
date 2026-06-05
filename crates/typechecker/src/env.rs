use std::collections::HashMap;

use ast::SmolStr;

use crate::types::{PolyType, TypeId};

/// A scoped type environment for tracking type bindings.
///
/// Supports nested scopes via `push_scope` / `pop_scope`.
/// Lookups search from innermost to outermost scope.
#[derive(Debug, Clone)]
pub struct TypeEnv {
    scopes: Vec<HashMap<SmolStr, PolyType>>,
    next_type_id: u32,
}

impl TypeEnv {
    /// Creates a new type environment with a single global scope.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            next_type_id: 0,
        }
    }

    /// Allocates a fresh type variable.
    pub fn fresh_var(&mut self) -> TypeId {
        let id = TypeId(self.next_type_id);
        self.next_type_id += 1;
        id
    }

    /// Pushes a new empty scope.
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pops the innermost scope.
    ///
    /// # Panics
    ///
    /// Panics if there is only one scope left (the global scope).
    pub fn pop_scope(&mut self) {
        assert!(self.scopes.len() > 1, "cannot pop the global scope");
        self.scopes.pop();
    }

    /// Inserts a type binding in the current (innermost) scope.
    pub fn insert(&mut self, name: impl Into<SmolStr>, ty: PolyType) {
        self.scopes
            .last_mut()
            .expect("scope stack is never empty")
            .insert(name.into(), ty);
    }

    /// Looks up a type by name, searching from innermost to outermost scope.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<&PolyType> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }

    /// Returns true if the given name is bound in any scope.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    /// Returns the number of active scopes.
    #[must_use]
    pub fn scope_depth(&self) -> usize {
        self.scopes.len()
    }

    /// Loads the prelude into the global scope.
    ///
    /// The prelude contains core types (`Option`, `Result`) and utility
    /// functions (`id`, `const`, `flip`, `compose`, `pipe`, `apply`)
    /// that are automatically available in every pipe-lang program.
    pub fn load_prelude(&mut self) {
        use crate::types::MonoType;

        // Load Option<T> as a sum type
        let opt_a = self.fresh_var();
        let option_type = PolyType::poly(
            vec![opt_a],
            MonoType::Tag {
                name: "Option".into(),
                payload: vec![MonoType::Var(opt_a)],
            },
        );
        self.insert("Option", option_type);

        // Load Result<T, E> as a sum type
        let res_t = self.fresh_var();
        let res_e = self.fresh_var();
        let result_type = PolyType::poly(
            vec![res_t, res_e],
            MonoType::Tag {
                name: "Result".into(),
                payload: vec![MonoType::Var(res_t), MonoType::Var(res_e)],
            },
        );
        self.insert("Result", result_type);

        // Load core utility function types
        // id : <a>(a) -> a
        let a = self.fresh_var();
        let id_type = PolyType::poly(
            vec![a],
            MonoType::Func {
                params: vec![MonoType::Var(a)],
                ret: Box::new(MonoType::Var(a)),
            },
        );
        self.insert("id", id_type);

        // const : <a, b>(a) -> (b) -> a
        let ca = self.fresh_var();
        let cb = self.fresh_var();
        let const_type = PolyType::poly(
            vec![ca, cb],
            MonoType::Func {
                params: vec![MonoType::Var(ca)],
                ret: Box::new(MonoType::Func {
                    params: vec![MonoType::Var(cb)],
                    ret: Box::new(MonoType::Var(ca)),
                }),
            },
        );
        self.insert("const", const_type);

        // flip : <a, b, c>((a, b) -> c) -> (b, a) -> c
        let fa = self.fresh_var();
        let fb = self.fresh_var();
        let fc = self.fresh_var();
        let flip_type = PolyType::poly(
            vec![fa, fb, fc],
            MonoType::Func {
                params: vec![MonoType::Func {
                    params: vec![MonoType::Var(fa), MonoType::Var(fb)],
                    ret: Box::new(MonoType::Var(fc)),
                }],
                ret: Box::new(MonoType::Func {
                    params: vec![MonoType::Var(fb), MonoType::Var(fa)],
                    ret: Box::new(MonoType::Var(fc)),
                }),
            },
        );
        self.insert("flip", flip_type);

        // compose : <a, b, c>((b) -> c, (a) -> b) -> (a) -> c
        let comp_a = self.fresh_var();
        let comp_b = self.fresh_var();
        let comp_c = self.fresh_var();
        let compose_type = PolyType::poly(
            vec![comp_a, comp_b, comp_c],
            MonoType::Func {
                params: vec![
                    MonoType::Func {
                        params: vec![MonoType::Var(comp_b)],
                        ret: Box::new(MonoType::Var(comp_c)),
                    },
                    MonoType::Func {
                        params: vec![MonoType::Var(comp_a)],
                        ret: Box::new(MonoType::Var(comp_b)),
                    },
                ],
                ret: Box::new(MonoType::Func {
                    params: vec![MonoType::Var(comp_a)],
                    ret: Box::new(MonoType::Var(comp_c)),
                }),
            },
        );
        self.insert("compose", compose_type);

        // pipe : <a, b>((a) -> b, (b) -> c) -> (a) -> c
        // Simplified: <a, b>((a) -> b) -> ... (chained, variadic in practice)
        // For now, just declare it as a generic function type
        let pipe_a = self.fresh_var();
        let pipe_b = self.fresh_var();
        let pipe_c = self.fresh_var();
        let pipe_type = PolyType::poly(
            vec![pipe_a, pipe_b, pipe_c],
            MonoType::Func {
                params: vec![
                    MonoType::Func {
                        params: vec![MonoType::Var(pipe_a)],
                        ret: Box::new(MonoType::Var(pipe_b)),
                    },
                    MonoType::Func {
                        params: vec![MonoType::Var(pipe_b)],
                        ret: Box::new(MonoType::Var(pipe_c)),
                    },
                ],
                ret: Box::new(MonoType::Func {
                    params: vec![MonoType::Var(pipe_a)],
                    ret: Box::new(MonoType::Var(pipe_c)),
                }),
            },
        );
        self.insert("pipe", pipe_type);

        // apply : <a, b>((a) -> b, a) -> b
        let app_a = self.fresh_var();
        let app_b = self.fresh_var();
        let apply_type = PolyType::poly(
            vec![app_a, app_b],
            MonoType::Func {
                params: vec![
                    MonoType::Func {
                        params: vec![MonoType::Var(app_a)],
                        ret: Box::new(MonoType::Var(app_b)),
                    },
                    MonoType::Var(app_a),
                ],
                ret: Box::new(MonoType::Var(app_b)),
            },
        );
        self.insert("apply", apply_type);
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MonoType;

    #[test]
    fn lookup_in_global_scope() {
        let mut env = TypeEnv::new();
        env.insert("x", PolyType::mono(MonoType::I32));
        assert_eq!(env.lookup("x"), Some(&PolyType::mono(MonoType::I32)));
    }

    #[test]
    fn lookup_in_inner_scope() {
        let mut env = TypeEnv::new();
        env.insert("x", PolyType::mono(MonoType::I32));
        env.push_scope();
        env.insert("y", PolyType::mono(MonoType::Str));
        assert_eq!(env.lookup("y"), Some(&PolyType::mono(MonoType::Str)));
        assert_eq!(env.lookup("x"), Some(&PolyType::mono(MonoType::I32)));
    }

    #[test]
    fn pop_scope_removes_bindings() {
        let mut env = TypeEnv::new();
        env.push_scope();
        env.insert("x", PolyType::mono(MonoType::I32));
        assert!(env.contains("x"));
        env.pop_scope();
        assert!(!env.contains("x"));
    }

    #[test]
    fn inner_scope_shadows_outer() {
        let mut env = TypeEnv::new();
        env.insert("x", PolyType::mono(MonoType::I32));
        env.push_scope();
        env.insert("x", PolyType::mono(MonoType::F64));
        assert_eq!(env.lookup("x"), Some(&PolyType::mono(MonoType::F64)));
        env.pop_scope();
        assert_eq!(env.lookup("x"), Some(&PolyType::mono(MonoType::I32)));
    }

    #[test]
    fn fresh_var_increments() {
        let mut env = TypeEnv::new();
        let v1 = env.fresh_var();
        let v2 = env.fresh_var();
        assert_ne!(v1, v2);
    }

    #[test]
    fn scope_depth_tracks_correctly() {
        let mut env = TypeEnv::new();
        assert_eq!(env.scope_depth(), 1);
        env.push_scope();
        assert_eq!(env.scope_depth(), 2);
        env.pop_scope();
        assert_eq!(env.scope_depth(), 1);
    }

    #[test]
    fn load_prelude_adds_option() {
        let mut env = TypeEnv::new();
        env.load_prelude();
        assert!(env.contains("Option"));
    }

    #[test]
    fn load_prelude_adds_result() {
        let mut env = TypeEnv::new();
        env.load_prelude();
        assert!(env.contains("Result"));
    }

    #[test]
    fn load_prelude_adds_core_functions() {
        let mut env = TypeEnv::new();
        env.load_prelude();
        assert!(env.contains("id"));
        assert!(env.contains("const"));
        assert!(env.contains("flip"));
        assert!(env.contains("compose"));
        assert!(env.contains("pipe"));
        assert!(env.contains("apply"));
    }

    #[test]
    fn load_prelude_id_has_correct_type() {
        let mut env = TypeEnv::new();
        env.load_prelude();
        let id_type = env.lookup("id").unwrap();
        assert!(id_type.quantified.len() == 1); // <a>
        assert!(matches!(id_type.body, MonoType::Func { .. }));
    }
}
