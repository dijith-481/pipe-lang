use std::collections::BTreeMap;
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
    // -- Signed integers --
    /// 8-bit signed integer.
    I8(i8),
    /// 16-bit signed integer.
    I16(i16),
    /// 32-bit signed integer.
    I32(i32),
    /// 64-bit signed integer.
    I64(i64),

    // -- Unsigned integers --
    /// 8-bit unsigned integer.
    U8(u8),
    /// 16-bit unsigned integer.
    U16(u16),
    /// 32-bit unsigned integer.
    U32(u32),
    /// 64-bit unsigned integer.
    U64(u64),
    /// Platform-dependent unsigned integer (used for indexing/sizes).
    Usize(usize),

    // -- Floats --
    /// 32-bit IEEE 754 float.
    F32(f32),
    /// 64-bit IEEE 754 float.
    F64(f64),

    // -- Other primitives --
    /// Boolean value.
    Bool(bool),
    /// Immutable string, reference-counted for cheap cloning.
    Str(SmolStr),

    // -- Compound --
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
    pub fields: BTreeMap<SmolStr, Value>,
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
    ///
    /// Numeric types are truthy if non-zero. Strings are truthy if non-empty.
    /// Arrays are truthy if non-empty. Unit is falsy. Closures, tags, and
    /// effects are always truthy.
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::I8(n) => *n != 0,
            Value::I16(n) => *n != 0,
            Value::I32(n) => *n != 0,
            Value::I64(n) => *n != 0,
            Value::U8(n) => *n != 0,
            Value::U16(n) => *n != 0,
            Value::U32(n) => *n != 0,
            Value::U64(n) => *n != 0,
            Value::Usize(n) => *n != 0,
            Value::F32(f) => *f != 0.0,
            Value::F64(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Unit => false,
            _ => true,
        }
    }

    /// Attempt to extract a 32-bit signed integer value.
    #[must_use]
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Value::I32(n) => Some(*n),
            _ => None,
        }
    }

    /// Attempt to extract a 64-bit signed integer value.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::I64(n) => Some(*n),
            _ => None,
        }
    }

    /// Attempt to extract a 64-bit float value.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::F64(f) => Some(*f),
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

    /// Returns true if this value is a numeric type of any width.
    #[must_use]
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Value::I8(_)
                | Value::I16(_)
                | Value::I32(_)
                | Value::I64(_)
                | Value::U8(_)
                | Value::U16(_)
                | Value::U32(_)
                | Value::U64(_)
                | Value::Usize(_)
                | Value::F32(_)
                | Value::F64(_)
        )
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

    /// Create a record value from a map of fields.
    pub fn record(fields: BTreeMap<SmolStr, Value>) -> Self {
        Value::Record(Arc::new(RecordData { fields }))
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::I8(a), Value::I8(b)) => a == b,
            (Value::I16(a), Value::I16(b)) => a == b,
            (Value::I32(a), Value::I32(b)) => a == b,
            (Value::I64(a), Value::I64(b)) => a == b,
            (Value::U8(a), Value::U8(b)) => a == b,
            (Value::U16(a), Value::U16(b)) => a == b,
            (Value::U32(a), Value::U32(b)) => a == b,
            (Value::U64(a), Value::U64(b)) => a == b,
            (Value::Usize(a), Value::Usize(b)) => a == b,
            (Value::F32(a), Value::F32(b)) => a == b,
            (Value::F64(a), Value::F64(b)) => a == b,
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
            Value::I8(n) => write!(f, "{n}i8"),
            Value::I16(n) => write!(f, "{n}i16"),
            Value::I32(n) => write!(f, "{n}"),
            Value::I64(n) => write!(f, "{n}i64"),
            Value::U8(n) => write!(f, "{n}u8"),
            Value::U16(n) => write!(f, "{n}u16"),
            Value::U32(n) => write!(f, "{n}u32"),
            Value::U64(n) => write!(f, "{n}u64"),
            Value::Usize(n) => write!(f, "{n}usize"),
            Value::F32(n) => write!(f, "{n}f32"),
            Value::F64(n) => write!(f, "{n}"),
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
    fn i32_value_roundtrip() {
        let v = Value::I32(42);
        assert_eq!(v.as_i32(), Some(42));
        assert_eq!(v.as_f64(), None);
    }

    #[test]
    fn i64_value_roundtrip() {
        let v = Value::I64(42);
        assert_eq!(v.as_i64(), Some(42));
    }

    #[test]
    fn f64_value_roundtrip() {
        let v = Value::F64(3.15);
        assert_eq!(v.as_f64(), Some(3.15));
        assert_eq!(v.as_i32(), None);
    }

    #[test]
    fn f32_value_roundtrip() {
        let v = Value::F32(2.71);
        match v {
            Value::F32(f) => assert!((f - 2.71).abs() < f32::EPSILON),
            _ => panic!("expected F32"),
        }
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
        let v = Value::array(vec![Value::I32(1), Value::I32(2), Value::I32(3)]);
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
        let v = Value::tag(0, vec![Value::I32(42)]);
        let (tag, payload) = v.as_tag().unwrap();
        assert_eq!(tag, 0);
        assert_eq!(payload[0].as_i32(), Some(42));
    }

    #[test]
    fn record_value_fields() {
        let mut fields = BTreeMap::new();
        fields.insert(SmolStr::new("name"), Value::Str(SmolStr::new("Alice")));
        fields.insert(SmolStr::new("age"), Value::I32(30));
        let v = Value::record(fields);
        match v {
            Value::Record(r) => {
                assert_eq!(r.fields.len(), 2);
                assert!(r.fields.contains_key("name"));
            }
            _ => panic!("expected Record"),
        }
    }

    #[test]
    fn value_equality_i32() {
        assert_eq!(Value::I32(5), Value::I32(5));
        assert_ne!(Value::I32(5), Value::I32(6));
    }

    #[test]
    fn value_equality_cross_type() {
        // Different numeric types are not equal
        assert_ne!(Value::I32(5), Value::F64(5.0));
        assert_ne!(Value::I32(0), Value::Bool(false));
        assert_ne!(Value::I32(5), Value::I64(5));
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
    fn value_debug_i32() {
        let v = Value::I32(42);
        assert_eq!(format!("{v:?}"), "42");
    }

    #[test]
    fn value_debug_i64() {
        let v = Value::I64(42);
        assert_eq!(format!("{v:?}"), "42i64");
    }

    #[test]
    fn value_debug_u8() {
        let v = Value::U8(255);
        assert_eq!(format!("{v:?}"), "255u8");
    }

    #[test]
    fn value_debug_f32() {
        let v = Value::F32(1.5);
        assert_eq!(format!("{v:?}"), "1.5f32");
    }

    #[test]
    fn value_debug_array() {
        let v = Value::array(vec![Value::I32(1), Value::I32(2)]);
        assert_eq!(format!("{v:?}"), "[1, 2]");
    }

    #[test]
    fn zero_i32_is_falsy() {
        let v = Value::I32(0);
        assert!(!v.is_truthy());
    }

    #[test]
    fn nonzero_i32_is_truthy() {
        let v = Value::I32(1);
        assert!(v.is_truthy());
    }

    #[test]
    fn is_numeric_true() {
        assert!(Value::I32(0).is_numeric());
        assert!(Value::F64(0.0).is_numeric());
        assert!(Value::U8(0).is_numeric());
        assert!(Value::Usize(0).is_numeric());
    }

    #[test]
    fn is_numeric_false() {
        assert!(!Value::Bool(true).is_numeric());
        assert!(!Value::Str(SmolStr::new("hi")).is_numeric());
        assert!(!Value::Unit.is_numeric());
    }
}
