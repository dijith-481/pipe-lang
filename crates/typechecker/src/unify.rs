use std::collections::BTreeMap;
use std::rc::Rc;

use ena::unify::{InPlaceUnificationTable, NoError, UnifyKey, UnifyValue};

use crate::error::TypeError;
use crate::types::{MonoType, TypeId};

// ---------------------------------------------------------------------------
// ena trait impls
// ---------------------------------------------------------------------------

impl UnifyKey for TypeId {
    type Value = Binding;

    fn index(&self) -> u32 {
        self.0
    }

    fn from_index(u: u32) -> Self {
        TypeId(u)
    }

    fn tag() -> &'static str {
        "TypeId"
    }
}

/// Newtype wrapping `Option<MonoType>` to satisfy the orphan rule for `UnifyValue`.
///
/// `None` = unbound type variable; `Some(t)` = resolved to `t`.
#[derive(Debug, Clone, Default)]
pub struct Binding(pub Option<MonoType>);

impl UnifyValue for Binding {
    type Error = NoError;

    fn unify_values(a: &Self, b: &Self) -> Result<Self, NoError> {
        Ok(match (&a.0, &b.0) {
            (None, x) | (x, None) => Binding(x.clone()),
            // Both bound — caller ensures inner types are already unified.
            (Some(x), Some(_)) => Binding(Some(x.clone())),
        })
    }
}

// ---------------------------------------------------------------------------
// Substitution (Union-Find table)
// ---------------------------------------------------------------------------

/// A Union-Find substitution table.
///
/// Type variables are looked up in amortized O(α(N)) time via path compression.
/// Binding a variable is a single in-place mutation — there is no O(N) re-walk.
#[derive(Default)]
pub struct Substitution {
    table: InPlaceUnificationTable<TypeId>,
}

impl Substitution {
    /// Creates a new, empty substitution table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ensures `id` is a valid key in the table, allocating unbound slots as needed.
    pub fn ensure_key(&mut self, id: TypeId) {
        while self.table.len() as u32 <= id.0 {
            self.table.new_key(Binding(None));
        }
    }

    /// Binds type variable `var` to `ty`.
    pub fn insert(&mut self, var: TypeId, ty: MonoType) {
        self.ensure_key(var);
        self.table.union_value(var, Binding(Some(ty)));
    }

    /// Resolves a type variable to its root binding, or `None` if unbound.
    pub fn lookup(&mut self, var: TypeId) -> Option<MonoType> {
        self.ensure_key(var);
        self.table.probe_value(var).0
    }

    /// Applies this substitution to `ty`, fully resolving all reachable type variables.
    #[must_use]
    pub fn apply(&mut self, ty: &MonoType) -> MonoType {
        match ty {
            MonoType::Var(id) => match self.lookup(*id) {
                Some(resolved) => self.apply(&resolved.clone()),
                None => ty.clone(),
            },
            MonoType::Array(inner) => MonoType::Array(Rc::new(self.apply(inner))),
            MonoType::Func { params, ret } => {
                let params: Vec<_> = params.iter().map(|p| self.apply(p)).collect();
                let ret = self.apply(ret);
                MonoType::Func { params: Rc::from(params.as_slice()), ret: Rc::new(ret) }
            }
            MonoType::Record(fields) => {
                let fields: BTreeMap<_, _> =
                    fields.iter().map(|(n, t)| (n.clone(), self.apply(t))).collect();
                MonoType::Record(Rc::new(fields))
            }
            MonoType::Tag { name, payload } => {
                let payload: Vec<_> = payload.iter().map(|t| self.apply(t)).collect();
                MonoType::Tag { name: name.clone(), payload: Rc::from(payload.as_slice()) }
            }
            _ => ty.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Occurs check
// ---------------------------------------------------------------------------

fn occurs_in(sub: &mut Substitution, var: TypeId, ty: &MonoType) -> bool {
    match ty {
        MonoType::Var(id) => {
            sub.ensure_key(*id);
            match sub.table.probe_value(*id).0 {
                Some(resolved) => {
                    let resolved = resolved.clone();
                    occurs_in(sub, var, &resolved)
                }
                None => sub.table.find(*id) == sub.table.find(var),
            }
        }
        MonoType::Array(inner) => occurs_in(sub, var, inner),
        MonoType::Func { params, ret } => {
            params.iter().any(|p| occurs_in(sub, var, p)) || occurs_in(sub, var, ret)
        }
        MonoType::Record(fields) => fields.values().any(|t| occurs_in(sub, var, t)),
        MonoType::Tag { payload, .. } => payload.iter().any(|t| occurs_in(sub, var, t)),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Unification (mutates the shared table in place)
// ---------------------------------------------------------------------------

/// Unifies types `a` and `b` by mutating the shared substitution table.
///
/// Because the table is shared, every subsequent `apply` or `lookup` call
/// immediately sees the new bindings — no substitution composition needed.
///
/// # Errors
///
/// - [`TypeError::UnificationFailed`] — structurally incompatible types.
/// - [`TypeError::InfiniteType`] — occurs check failure (would create an infinite type).
/// - [`TypeError::ArityMismatch`] — functions with different parameter counts.
pub fn unify(sub: &mut Substitution, a: &MonoType, b: &MonoType) -> Result<(), TypeError> {
    // Walk through any existing bindings before comparing.
    let a = sub.apply(a);
    let b = sub.apply(b);

    match (&a, &b) {
        _ if a == b => Ok(()),

        (MonoType::Var(id), _) => bind(sub, *id, &b),
        (_, MonoType::Var(id)) => bind(sub, *id, &a),

        (MonoType::Array(ai), MonoType::Array(bi)) => {
            let ai = ai.clone();
            let bi = bi.clone();
            unify(sub, &ai, &bi)
        }

        (
            MonoType::Func { params: ap, ret: ar },
            MonoType::Func { params: bp, ret: br },
        ) => {
            if ap.len() != bp.len() {
                return Err(TypeError::ArityMismatch {
                    expected: ap.len(),
                    got: bp.len(),
                    span: ast::span::Span::new(0, 0),
                });
            }
            // Clone Rcs upfront to avoid borrow-while-mutating issues.
            let pairs: Vec<_> = ap.iter().cloned().zip(bp.iter().cloned()).collect();
            let ar = ar.clone();
            let br = br.clone();
            for (p, q) in pairs {
                unify(sub, &p, &q)?;
            }
            unify(sub, &ar, &br)
        }

        (MonoType::Record(af), MonoType::Record(bf)) => {
            if af.len() != bf.len() {
                return Err(mismatch(&a, &b));
            }
            let pairs: Vec<_> = af
                .iter()
                .map(|(name, at)| {
                    let bt = bf.get(name).ok_or_else(|| mismatch(&a, &b))?;
                    Ok((at.clone(), bt.clone()))
                })
                .collect::<Result<_, TypeError>>()?;
            for (at, bt) in pairs {
                unify(sub, &at, &bt)?;
            }
            Ok(())
        }

        (MonoType::Tag { name: an, payload: ap }, MonoType::Tag { name: bn, payload: bp }) => {
            if an != bn || ap.len() != bp.len() {
                return Err(mismatch(&a, &b));
            }
            let pairs: Vec<_> = ap.iter().cloned().zip(bp.iter().cloned()).collect();
            for (at, bt) in pairs {
                unify(sub, &at, &bt)?;
            }
            Ok(())
        }

        _ => Err(mismatch(&a, &b)),
    }
}

fn bind(sub: &mut Substitution, id: TypeId, ty: &MonoType) -> Result<(), TypeError> {
    if let MonoType::Var(other) = ty {
        sub.ensure_key(*other);
        sub.ensure_key(id);
        if sub.table.find(id) == sub.table.find(*other) {
            return Ok(());
        }
    }
    if occurs_in(sub, id, ty) {
        return Err(TypeError::InfiniteType {
            var: id,
            ty: ty.clone(),
            span: ast::span::Span::new(0, 0),
        });
    }
    sub.insert(id, ty.clone());
    Ok(())
}

#[inline]
fn mismatch(a: &MonoType, b: &MonoType) -> TypeError {
    TypeError::UnificationFailed {
        expected: a.clone(),
        got: b.clone(),
        span: ast::span::Span::new(0, 0),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TypeId;

    fn var(n: u32) -> MonoType {
        MonoType::Var(TypeId(n))
    }

    fn fresh(sub: &mut Substitution, id: u32) -> MonoType {
        sub.ensure_key(TypeId(id));
        var(id)
    }

    #[test]
    fn unify_same_concrete() {
        let mut sub = Substitution::new();
        unify(&mut sub, &MonoType::I32, &MonoType::I32).unwrap();
        assert_eq!(sub.apply(&MonoType::I32), MonoType::I32);
    }

    #[test]
    fn unify_var_with_concrete() {
        let mut sub = Substitution::new();
        let v = fresh(&mut sub, 0);
        unify(&mut sub, &v, &MonoType::I32).unwrap();
        assert_eq!(sub.apply(&v), MonoType::I32);
    }

    #[test]
    fn unify_concrete_with_var() {
        let mut sub = Substitution::new();
        let v = fresh(&mut sub, 0);
        unify(&mut sub, &MonoType::Str, &v).unwrap();
        assert_eq!(sub.apply(&v), MonoType::Str);
    }

    #[test]
    fn unify_mismatched_fails() {
        let mut sub = Substitution::new();
        assert!(matches!(
            unify(&mut sub, &MonoType::I32, &MonoType::Str),
            Err(TypeError::UnificationFailed { .. })
        ));
    }

    #[test]
    fn unify_arrays() {
        let mut sub = Substitution::new();
        let v = fresh(&mut sub, 0);
        let a = MonoType::Array(Rc::new(MonoType::I32));
        let b = MonoType::Array(Rc::new(v.clone()));
        unify(&mut sub, &a, &b).unwrap();
        assert_eq!(sub.apply(&v), MonoType::I32);
    }

    #[test]
    fn unify_functions() {
        let mut sub = Substitution::new();
        let v0 = fresh(&mut sub, 0);
        let v1 = fresh(&mut sub, 1);
        let a = MonoType::Func {
            params: Rc::from([MonoType::I32]),
            ret: Rc::new(MonoType::Bool),
        };
        let b = MonoType::Func {
            params: Rc::from([v0.clone()]),
            ret: Rc::new(v1.clone()),
        };
        unify(&mut sub, &a, &b).unwrap();
        assert_eq!(sub.apply(&v0), MonoType::I32);
        assert_eq!(sub.apply(&v1), MonoType::Bool);
    }

    #[test]
    fn unify_arity_mismatch() {
        let mut sub = Substitution::new();
        let a = MonoType::Func {
            params: Rc::from([MonoType::I32]),
            ret: Rc::new(MonoType::Bool),
        };
        let b = MonoType::Func {
            params: Rc::from([MonoType::I32, MonoType::Str]),
            ret: Rc::new(MonoType::Bool),
        };
        assert!(matches!(unify(&mut sub, &a, &b), Err(TypeError::ArityMismatch { .. })));
    }

    #[test]
    fn chain_resolution() {
        // ?0 → ?1 → i32  should resolve to i32
        let mut sub = Substitution::new();
        let v0 = fresh(&mut sub, 0);
        let v1 = fresh(&mut sub, 1);
        unify(&mut sub, &v0, &v1).unwrap();
        unify(&mut sub, &v1, &MonoType::I32).unwrap();
        assert_eq!(sub.apply(&v0), MonoType::I32);
    }

    #[test]
    fn occurs_check_infinite_type() {
        let mut sub = Substitution::new();
        let v = fresh(&mut sub, 0);
        let arr = MonoType::Array(Rc::new(v.clone()));
        assert!(matches!(unify(&mut sub, &v, &arr), Err(TypeError::InfiniteType { .. })));
    }

    #[test]
    fn unify_records() {
        let mut sub = Substitution::new();
        let v = fresh(&mut sub, 0);
        let mut fa = BTreeMap::new();
        fa.insert("x".into(), MonoType::I32);
        let mut fb = BTreeMap::new();
        fb.insert("x".into(), v.clone());
        unify(&mut sub, &MonoType::Record(Rc::new(fa)), &MonoType::Record(Rc::new(fb))).unwrap();
        assert_eq!(sub.apply(&v), MonoType::I32);
    }

    #[test]
    fn unify_tags_same() {
        let mut sub = Substitution::new();
        let v = fresh(&mut sub, 0);
        let a = MonoType::Tag { name: "Some".into(), payload: Rc::from([MonoType::I32]) };
        let b = MonoType::Tag { name: "Some".into(), payload: Rc::from([v.clone()]) };
        unify(&mut sub, &a, &b).unwrap();
        assert_eq!(sub.apply(&v), MonoType::I32);
    }

    #[test]
    fn unify_tags_different_names_fails() {
        let mut sub = Substitution::new();
        let a = MonoType::Tag { name: "Some".into(), payload: Rc::from([MonoType::I32]) };
        let b = MonoType::Tag { name: "None".into(), payload: Rc::from([MonoType::I32]) };
        assert!(matches!(unify(&mut sub, &a, &b), Err(TypeError::UnificationFailed { .. })));
    }
}
