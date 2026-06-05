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
}
