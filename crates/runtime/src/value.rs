use std::fmt;
use std::sync::Arc;

use ast::SmolStr;

use crate::bridge::BuiltinFunction;

/// Runtime values in the language.
///
/// All values are immutable and reference-counted for safe sharing
/// without a garbage collector. Heap allocations use `Arc` for
/// deterministic cleanup.
#[derive(Clone)]
pub enum Value {
    /// 64-bit signed integer.
    Int(i64),
    /// 64-bit IEEE 754 float.
    Float(f64),
    /// Boolean value.
    Bool(bool),
    /// Immutable string, reference-counted for cheap cloning.
    Str(SmolStr),
    /// Immutable array, reference-counted.
    Array(Arc<[Value]>),
    /// Record with named fields.
    Record(Arc<RecordData>),
    /// Closure capturing an environment.
    Closure(Arc<ClosureData>),
    /// A tagged union variant (sum type).
    Tag { tag: u32, payload: Arc<[Value]> },
    /// An effectful computation (IO, etc.) wrapping a builtin.
    Effect(Arc<dyn BuiltinFunction>),
    /// Unit value (empty tuple).
    Unit,
}

/// Data for a record value.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordData {
    pub fields: Vec<(SmolStr, Value)>,
}

/// Data for a closure value.
#[derive(Debug, Clone)]
pub struct ClosureData {
    pub func: FuncPtr,
    pub captures: Arc<[Value]>,
    pub arity: usize,
}

/// A pointer to a function (either a builtin or a JIT-compiled function).
#[derive(Debug, Clone)]
pub enum FuncPtr {
    /// A built-in function implemented in Rust.
    Builtin(Arc<dyn BuiltinFunction>),
    /// A JIT-compiled function pointer (address + metadata).
    Jit { address: usize, arity: usize },
}

impl Value {
    /// Returns true if this value is the unit value.
    #[must_use]
    pub fn is_unit(&self) -> bool {
        matches!(self, Value::Unit)
    }

    /// Returns true if this value is truthy (for boolean contexts).
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Unit => false,
            _ => true,
        }
    }

    /// Attempt to extract an integer value.
    #[must_use]
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// Attempt to extract a float value.
    #[must_use]
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Attempt to extract a boolean value.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Attempt to extract a string slice.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Attempt to extract an array slice.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Attempt to extract a tag variant and payload.
    #[must_use]
    pub fn as_tag(&self) -> Option<(u32, &[Value])> {
        match self {
            Value::Tag { tag, payload } => Some((*tag, payload)),
            _ => None,
        }
    }

    /// Create an array value from a vector.
    pub fn array(values: Vec<Value>) -> Self {
        Value::Array(Arc::from(values))
    }

    /// Create a tag value.
    pub fn tag(tag: u32, payload: Vec<Value>) -> Self {
        Value::Tag {
            tag,
            payload: Arc::from(payload),
        }
    }

    /// Create a record value from a list of field pairs.
    pub fn record(fields: Vec<(SmolStr, Value)>) -> Self {
        Value::Record(Arc::new(RecordData { fields }))
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Unit, Value::Unit) => true,
            (Value::Record(a), Value::Record(b)) => a == b,
            (
                Value::Tag {
                    tag: t1,
                    payload: p1,
                },
                Value::Tag {
                    tag: t2,
                    payload: p2,
                },
            ) => t1 == t2 && p1 == p2,
            _ => false,
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "\"{s}\""),
            Value::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v:?}")?;
                }
                write!(f, "]")
            }
            Value::Record(r) => {
                write!(f, "{{ ")?;
                for (i, (name, val)) in r.fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{name}: {val:?}")?;
                }
                write!(f, " }}")
            }
            Value::Closure(c) => {
                write!(f, "<closure/{}>", c.arity)
            }
            Value::Tag { tag, payload } => {
                write!(f, "Tag({tag}")?;
                for v in payload.iter() {
                    write!(f, ", {v:?}")?;
                }
                write!(f, ")")
            }
            Value::Effect(_) => write!(f, "<effect>"),
            Value::Unit => write!(f, "()"),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Str(s) => write!(f, "{s}"),
            other => write!(f, "{other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_value_roundtrip() {
        let v = Value::Int(42);
        assert_eq!(v.as_int(), Some(42));
        assert_eq!(v.as_float(), None);
    }

    #[test]
    fn float_value_roundtrip() {
        let v = Value::Float(3.14);
        assert_eq!(v.as_float(), Some(3.14));
        assert_eq!(v.as_int(), None);
    }

    #[test]
    fn bool_value_roundtrip() {
        let v = Value::Bool(true);
        assert_eq!(v.as_bool(), Some(true));
        assert!(v.is_truthy());

        let v = Value::Bool(false);
        assert_eq!(v.as_bool(), Some(false));
        assert!(!v.is_truthy());
    }

    #[test]
    fn str_value_roundtrip() {
        let v = Value::Str(SmolStr::new("hello"));
        assert_eq!(v.as_str(), Some("hello"));
        assert!(v.is_truthy());
    }

    #[test]
    fn empty_str_is_falsy() {
        let v = Value::Str(SmolStr::new(""));
        assert!(!v.is_truthy());
    }

    #[test]
    fn array_value_operations() {
        let v = Value::array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        assert_eq!(v.as_array().unwrap().len(), 3);
        assert!(v.is_truthy());
    }

    #[test]
    fn empty_array_is_falsy() {
        let v = Value::array(vec![]);
        assert!(!v.is_truthy());
    }

    #[test]
    fn unit_is_falsy() {
        let v = Value::Unit;
        assert!(v.is_unit());
        assert!(!v.is_truthy());
    }

    #[test]
    fn tag_value_roundtrip() {
        let v = Value::tag(0, vec![Value::Int(42)]);
        let (tag, payload) = v.as_tag().unwrap();
        assert_eq!(tag, 0);
        assert_eq!(payload[0].as_int(), Some(42));
    }

    #[test]
    fn record_value_fields() {
        let v = Value::record(vec![
            (SmolStr::new("name"), Value::Str(SmolStr::new("Alice"))),
            (SmolStr::new("age"), Value::Int(30)),
        ]);
        match v {
            Value::Record(r) => {
                assert_eq!(r.fields.len(), 2);
                assert_eq!(r.fields[0].0.as_str(), "name");
            }
            _ => panic!("expected Record"),
        }
    }

    #[test]
    fn value_equality_int() {
        assert_eq!(Value::Int(5), Value::Int(5));
        assert_ne!(Value::Int(5), Value::Int(6));
    }

    #[test]
    fn value_equality_cross_type() {
        assert_ne!(Value::Int(5), Value::Float(5.0));
        assert_ne!(Value::Int(0), Value::Bool(false));
    }

    #[test]
    fn value_clone_is_cheap() {
        let v = Value::Str(SmolStr::new("hello"));
        let v2 = v.clone();
        assert_eq!(v, v2);
    }

    #[test]
    fn value_display_str() {
        let v = Value::Str(SmolStr::new("hello"));
        assert_eq!(format!("{v}"), "hello");
    }

    #[test]
    fn value_debug_int() {
        let v = Value::Int(42);
        assert_eq!(format!("{v:?}"), "42");
    }

    #[test]
    fn value_debug_array() {
        let v = Value::array(vec![Value::Int(1), Value::Int(2)]);
        assert_eq!(format!("{v:?}"), "[1, 2]");
    }

    #[test]
    fn zero_int_is_falsy() {
        let v = Value::Int(0);
        assert!(!v.is_truthy());
    }

    #[test]
    fn nonzero_int_is_truthy() {
        let v = Value::Int(1);
        assert!(v.is_truthy());
    }
}
