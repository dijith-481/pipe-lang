use std::fmt;

use ast::SmolStr;

/// Unique identifier for a type variable.
///
/// Type variables are used during inference to represent unknown types
/// that will be resolved through unification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub u32);

impl fmt::Display for TypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?{}", self.0)
    }
}

/// Monomorphic (fully resolved) types.
///
/// These represent concrete types with no remaining type variables,
/// or type variables that haven't been resolved yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonoType {
    // -- Signed integers --
    I8,
    I16,
    I32,
    I64,

    // -- Unsigned integers --
    U8,
    U16,
    U32,
    U64,
    Usize,

    // -- Floats --
    F32,
    F64,

    // -- Other primitives --
    Bool,
    Str,

    // -- Compound --
    Array(Box<MonoType>),
    Func {
        params: Vec<MonoType>,
        ret: Box<MonoType>,
    },
    Record(Vec<(SmolStr, MonoType)>),
    Tag {
        name: SmolStr,
        payload: Vec<MonoType>,
    },
    Unit,

    // -- Type variable (unresolved) --
    Var(TypeId),
}

impl fmt::Display for MonoType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MonoType::I8 => write!(f, "i8"),
            MonoType::I16 => write!(f, "i16"),
            MonoType::I32 => write!(f, "i32"),
            MonoType::I64 => write!(f, "i64"),
            MonoType::U8 => write!(f, "u8"),
            MonoType::U16 => write!(f, "u16"),
            MonoType::U32 => write!(f, "u32"),
            MonoType::U64 => write!(f, "u64"),
            MonoType::Usize => write!(f, "usize"),
            MonoType::F32 => write!(f, "f32"),
            MonoType::F64 => write!(f, "f64"),
            MonoType::Bool => write!(f, "bool"),
            MonoType::Str => write!(f, "str"),
            MonoType::Unit => write!(f, "()"),
            MonoType::Var(id) => write!(f, "{id}"),
            MonoType::Array(inner) => write!(f, "[{inner}]"),
            MonoType::Func { params, ret } => {
                write!(f, "(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {ret}")
            }
            MonoType::Record(fields) => {
                write!(f, "{{")?;
                for (i, (name, ty)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{name}: {ty}")?;
                }
                write!(f, "}}")
            }
            MonoType::Tag { name, payload } => {
                write!(f, "{name}")?;
                if !payload.is_empty() {
                    write!(f, "(")?;
                    for (i, t) in payload.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{t}")?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
        }
    }
}

impl MonoType {
    /// Returns true if this type contains no type variables.
    #[must_use]
    pub fn is_concrete(&self) -> bool {
        match self {
            MonoType::Var(_) => false,
            MonoType::Array(inner) => inner.is_concrete(),
            MonoType::Func { params, ret } => {
                params.iter().all(|p| p.is_concrete()) && ret.is_concrete()
            }
            MonoType::Record(fields) => fields.iter().all(|(_, t)| t.is_concrete()),
            MonoType::Tag { payload, .. } => payload.iter().all(|t| t.is_concrete()),
            _ => true,
        }
    }

    /// Returns true if this type is a numeric type (any width/signedness).
    #[must_use]
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            MonoType::I8
                | MonoType::I16
                | MonoType::I32
                | MonoType::I64
                | MonoType::U8
                | MonoType::U16
                | MonoType::U32
                | MonoType::U64
                | MonoType::Usize
                | MonoType::F32
                | MonoType::F64
        )
    }
}

/// Polymorphic type (quantified over type variables).
///
/// Represents a type scheme like `∀a. a -> a` (the identity function).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolyType {
    /// Type variables quantified (universally bound) in this type.
    pub quantified: Vec<TypeId>,
    /// The body type.
    pub body: MonoType,
}

impl fmt::Display for PolyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.quantified.is_empty() {
            write!(f, "{}", self.body)
        } else {
            write!(f, "∀")?;
            for (i, v) in self.quantified.iter().enumerate() {
                if i > 0 {
                    write!(f, " ")?;
                }
                write!(f, "{v}")?;
            }
            write!(f, ". {}", self.body)
        }
    }
}

impl PolyType {
    /// Creates a monomorphic type (no quantified variables).
    #[must_use]
    pub fn mono(ty: MonoType) -> Self {
        Self {
            quantified: Vec::new(),
            body: ty,
        }
    }

    /// Creates a polymorphic type with the given quantified variables.
    #[must_use]
    pub fn poly(quantified: Vec<TypeId>, body: MonoType) -> Self {
        Self { quantified, body }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_type_is_concrete() {
        assert!(MonoType::I32.is_concrete());
        assert!(MonoType::Bool.is_concrete());
        assert!(MonoType::Array(Box::new(MonoType::Str)).is_concrete());
    }

    #[test]
    fn mono_type_with_var_is_not_concrete() {
        let ty = MonoType::Var(TypeId(0));
        assert!(!ty.is_concrete());
        assert!(!MonoType::Array(Box::new(ty)).is_concrete());
    }

    #[test]
    fn is_numeric_true() {
        assert!(MonoType::I32.is_numeric());
        assert!(MonoType::F64.is_numeric());
        assert!(MonoType::U8.is_numeric());
    }

    #[test]
    fn is_numeric_false() {
        assert!(!MonoType::Bool.is_numeric());
        assert!(!MonoType::Str.is_numeric());
        assert!(!MonoType::Unit.is_numeric());
    }

    #[test]
    fn poly_type_mono_helper() {
        let poly = PolyType::mono(MonoType::I32);
        assert!(poly.quantified.is_empty());
        assert_eq!(poly.body, MonoType::I32);
    }

    #[test]
    fn func_type_construction() {
        let func = MonoType::Func {
            params: vec![MonoType::I32, MonoType::Str],
            ret: Box::new(MonoType::Bool),
        };
        assert!(func.is_concrete());
    }
}
