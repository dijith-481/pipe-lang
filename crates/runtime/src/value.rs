use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};

use ast::SmolStr;

use crate::bridge::BuiltinFunction;

/// Global registry mapping JIT function addresses to their parameter types.
/// Populated by the JIT compiler, read by the stdlib when calling JIT closures.
static JIT_FUNC_PARAM_TYPES: OnceLock<Mutex<HashMap<usize, Vec<JitArgType>>>> = OnceLock::new();

/// Register parameter types for a JIT-compiled function.
pub fn register_jit_param_types(address: usize, param_types: Vec<JitArgType>) {
    let map = JIT_FUNC_PARAM_TYPES.get_or_init(|| Mutex::new(HashMap::new()));
    map.lock().unwrap().insert(address, param_types);
}

/// Look up parameter types for a JIT-compiled function.
pub fn lookup_jit_param_types(address: usize) -> Vec<JitArgType> {
    JIT_FUNC_PARAM_TYPES
        .get()
        .and_then(|m| m.lock().unwrap().get(&address).cloned())
        .unwrap_or_default()
}

/// A runtime value in pipe-lang.
///
/// All heap data is behind an [`Arc`] for deterministic, GC-free memory
/// management. Because the language is immutable, reference cycles are
/// structurally impossible, so `Arc` never leaks.
#[repr(C)]
#[derive(Clone)]
pub enum Value {
    /// A 32-bit signed integer.
    I32(i32),
    /// A 64-bit signed integer.
    I64(i64),
    /// A platform-native unsigned integer (usize).
    Usize(usize),
    /// A 64-bit floating point number.
    F64(f64),
    /// A boolean.
    Bool(bool),
    /// The unit value, `()`.
    Unit,
    /// A reference-counted UTF-8 string.
    Str(Arc<str>),
    /// A reference-counted immutable array.
    Array(Arc<[Value]>),
    /// A reference-counted record (product type).
    Record(Arc<RecordData>),
    /// A reference-counted closure.
    Closure(Arc<ClosureData>),
    /// A tagged-union value (sum type).
    Tag { tag: u32, payload: Arc<[Value]> },
    /// A deferred effectful computation (IO, etc.).
    Effect(Arc<dyn BuiltinFunction>),
}

// ---------------------------------------------------------------------------
// Constructor helpers
// ---------------------------------------------------------------------------

impl Value {
    /// Creates a string runtime value.
    #[must_use]
    pub fn str(s: impl Into<Arc<str>>) -> Self {
        Self::Str(s.into())
    }

    /// Creates an immutable array runtime value.
    #[must_use]
    pub fn array(values: Vec<Value>) -> Self {
        Self::Array(Arc::from(values.into_boxed_slice()))
    }

    /// Creates a record runtime value.
    #[must_use]
    pub fn record(fields: BTreeMap<SmolStr, Value>) -> Self {
        Self::Record(Arc::new(RecordData { fields }))
    }

    /// Creates a tagged-union runtime value.
    #[must_use]
    pub fn tag(tag: u32, payload: Vec<Value>) -> Self {
        Self::Tag {
            tag,
            payload: Arc::from(payload.into_boxed_slice()),
        }
    }
}

// ---------------------------------------------------------------------------
// Accessors
// ---------------------------------------------------------------------------

impl Value {
    /// Returns `true` if this is the unit value.
    #[must_use]
    pub fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }

    /// Returns `true` if this value is truthy in a boolean context.
    ///
    /// Numeric types are truthy if non-zero. Strings and arrays are truthy
    /// if non-empty. Unit is falsy. Tags, closures, records, and effects
    /// are always truthy.
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::I32(n) => *n != 0,
            Self::I64(n) => *n != 0,
            Self::Usize(n) => *n != 0,
            Self::F64(f) => *f != 0.0,
            Self::Str(s) => !s.is_empty(),
            Self::Array(a) => !a.is_empty(),
            Self::Unit => false,
            Self::Record(_) | Self::Closure(_) | Self::Tag { .. } | Self::Effect(_) => true,
        }
    }

    /// Returns `true` if this value is a numeric type.
    #[must_use]
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::I32(_) | Self::I64(_) | Self::Usize(_) | Self::F64(_)
        )
    }

    /// Returns the contained `i32`, if this is [`Value::I32`].
    #[must_use]
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Self::I32(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns the contained `i64`, if this is [`Value::I64`].
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::I64(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns the contained `f64`, if this is [`Value::F64`].
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::F64(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns the contained `bool`, if this is [`Value::Bool`].
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns the contained string slice, if this is [`Value::Str`].
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the contained array slice, if this is [`Value::Array`].
    #[must_use]
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(values) => Some(values),
            _ => None,
        }
    }

    /// Returns the tag discriminant and payload, if this is [`Value::Tag`].
    #[must_use]
    pub fn as_tag(&self) -> Option<(u32, &[Value])> {
        match self {
            Self::Tag { tag, payload } => Some((*tag, payload)),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// PartialEq — deep equality
// ---------------------------------------------------------------------------

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::I32(a), Self::I32(b)) => a == b,
            (Self::I64(a), Self::I64(b)) => a == b,
            (Self::Usize(a), Self::Usize(b)) => a == b,
            (Self::F64(a), Self::F64(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Unit, Self::Unit) => true,
            (Self::Str(a), Self::Str(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Record(a), Self::Record(b)) => a == b,
            (
                Self::Tag {
                    tag: t1,
                    payload: p1,
                },
                Self::Tag {
                    tag: t2,
                    payload: p2,
                },
            ) => t1 == t2 && p1 == p2,
            // Closures and Effects are compared by identity (pointer equality).
            // Trait objects cannot implement PartialEq safely.
            (Self::Closure(a), Self::Closure(b)) => Arc::ptr_eq(a, b),
            (Self::Effect(_), Self::Effect(_)) => false,
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Debug
// ---------------------------------------------------------------------------

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I32(n) => write!(f, "{n}"),
            Self::I64(n) => write!(f, "{n}i64"),
            Self::Usize(n) => write!(f, "{n}usize"),
            Self::F64(n) => write!(f, "{n}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Unit => write!(f, "()"),
            Self::Str(s) => write!(f, "\"{s}\""),
            Self::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v:?}")?;
                }
                write!(f, "]")
            }
            Self::Record(r) => {
                write!(f, "{{ ")?;
                for (i, (name, val)) in r.fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{name}: {val:?}")?;
                }
                write!(f, " }}")
            }
            Self::Closure(c) => write!(f, "<closure/{}>", c.arity),
            Self::Tag { tag, payload } => {
                write!(f, "Tag({tag}")?;
                for v in payload.iter() {
                    write!(f, ", {v:?}")?;
                }
                write!(f, ")")
            }
            Self::Effect(_) => write!(f, "<effect>"),
        }
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Str(s) => write!(f, "{s}"),
            other => write!(f, "{other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// Data stored by a record value.
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecordData {
    /// Record fields keyed by source-level field name.
    pub fields: BTreeMap<SmolStr, Value>,
}

/// A pointer to a function that can be called by the runtime.
#[derive(Debug, Clone)]
pub enum FuncPtr {
    /// A built-in function implemented in Rust.
    Builtin(Arc<dyn BuiltinFunction>),
    /// A JIT-compiled native function.
    Jit { address: usize, arity: usize },
}

/// Lightweight type tag for JIT calling convention.
/// Mirrors the IrType variants used by the JIT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum JitArgType {
    I8 = 0,
    I16 = 1,
    I32 = 2,
    I64 = 3,
    U8 = 4,
    U16 = 5,
    U32 = 6,
    U64 = 7,
    F32 = 8,
    F64 = 9,
    Bool = 10,
    Str = 11,
    Unit = 12,
    Array = 13,
    Record = 14,
    Effect = 15,
    Closure = 16,
    Tag = 17,
}

impl JitArgType {
    /// Size in bytes of the raw value in the JIT args buffer.
    pub fn raw_size(self) -> usize {
        match self {
            Self::I8 | Self::U8 | Self::Bool => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64
            | Self::U64
            | Self::F64
            | Self::Str
            | Self::Array
            | Self::Record
            | Self::Effect
            | Self::Closure
            | Self::Tag
            | Self::Unit => 8,
        }
    }

    /// Total slot size in the JIT args buffer (value + padding to 8-byte alignment).
    pub fn slot_size(self) -> usize {
        self.raw_size().max(8)
    }

    /// Convert from a Cranelift/IR type tag (as used in the JIT bridge).
    pub fn from_type_tag(tag: u32) -> Self {
        match tag {
            0 => Self::I8,
            1 => Self::I16,
            2 => Self::I32,
            3 => Self::I64,
            4 => Self::U8,
            5 => Self::U16,
            6 => Self::U32,
            7 => Self::U64,
            8 => Self::F32,
            9 => Self::F64,
            10 => Self::Bool,
            11 => Self::Str,
            12 => Self::Unit,
            13 => Self::Array,
            14 => Self::Record,
            15 => Self::Effect,
            16 => Self::Closure,
            17 => Self::Tag,
            _ => Self::Unit,
        }
    }
}

/// Data stored by a closure value.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ClosureData {
    /// The function pointer (builtin or JIT).
    pub func: FuncPtr,
    /// Values captured by the closure environment.
    pub captures: Arc<[Value]>,
    /// Number of arguments expected by the closure.
    pub arity: usize,
    /// Parameter types for JIT closures (empty for builtins).
    /// Used by the stdlib to serialize arguments when calling JIT closures.
    pub call_arg_types: Arc<[JitArgType]>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Dummy;
    impl BuiltinFunction for Dummy {
        fn name(&self) -> &str {
            "dummy"
        }
        fn arity(&self) -> usize {
            0
        }
        fn execute(&self, _args: &[Value]) -> Result<Value, String> {
            Ok(Value::Unit)
        }
    }

    // -- Constructor tests --

    #[test]
    fn value_str_constructor_stores_text() {
        let value = Value::str("hello");
        assert_eq!(value.as_str(), Some("hello"));
    }

    #[test]
    fn value_array_constructor_stores_values() {
        let value = Value::array(vec![Value::I32(1), Value::I32(2)]);
        assert_eq!(
            value.as_array(),
            Some([Value::I32(1), Value::I32(2)].as_slice())
        );
    }

    #[test]
    fn value_tag_constructor_stores_tag_and_payload() {
        let value = Value::tag(1, vec![Value::Bool(true)]);
        let (tag, payload) = value.as_tag().unwrap();
        assert_eq!(tag, 1);
        assert_eq!(payload, [Value::Bool(true)].as_slice());
    }

    #[test]
    fn value_record_constructor_stores_fields() {
        let mut fields = BTreeMap::new();
        fields.insert(SmolStr::new("answer"), Value::I32(42));
        let value = Value::record(fields);
        match value {
            Value::Record(ref record) => {
                assert_eq!(record.fields.get("answer"), Some(&Value::I32(42)));
            }
            _ => panic!("expected Record"),
        }
    }

    // -- Accessor tests --

    #[test]
    fn primitive_accessors_match_kept_types() {
        assert_eq!(Value::I32(1).as_i32(), Some(1));
        assert_eq!(Value::I64(2).as_i64(), Some(2));
        assert_eq!(Value::F64(3.5).as_f64(), Some(3.5));
        assert_eq!(Value::Bool(false).as_bool(), Some(false));
        assert_eq!(Value::Unit.as_i32(), None);
    }

    #[test]
    fn as_str_returns_none_for_non_string() {
        assert_eq!(Value::I32(0).as_str(), None);
        assert_eq!(Value::Unit.as_str(), None);
    }

    #[test]
    fn as_array_returns_none_for_non_array() {
        assert_eq!(Value::I32(0).as_array(), None);
    }

    #[test]
    fn as_tag_returns_none_for_non_tag() {
        assert_eq!(Value::Unit.as_tag(), None);
    }

    // -- Unit tests --

    #[test]
    fn is_unit_true_for_unit() {
        assert!(Value::Unit.is_unit());
    }

    #[test]
    fn is_unit_false_for_other_values() {
        assert!(!Value::I32(0).is_unit());
        assert!(!Value::Bool(true).is_unit());
    }

    // -- Truthiness tests --

    #[test]
    fn zero_i32_is_falsy() {
        assert!(!Value::I32(0).is_truthy());
    }

    #[test]
    fn nonzero_i32_is_truthy() {
        assert!(Value::I32(1).is_truthy());
    }

    #[test]
    fn zero_i64_is_falsy() {
        assert!(!Value::I64(0).is_truthy());
    }

    #[test]
    fn nonzero_i64_is_truthy() {
        assert!(Value::I64(1).is_truthy());
    }

    #[test]
    fn zero_f64_is_falsy() {
        assert!(!Value::F64(0.0).is_truthy());
    }

    #[test]
    fn nonzero_f64_is_truthy() {
        assert!(Value::F64(1.5).is_truthy());
    }

    #[test]
    fn bool_is_its_truthiness() {
        assert!(Value::Bool(true).is_truthy());
        assert!(!Value::Bool(false).is_truthy());
    }

    #[test]
    fn unit_is_falsy() {
        assert!(!Value::Unit.is_truthy());
    }

    #[test]
    fn empty_str_is_falsy() {
        assert!(!Value::str("").is_truthy());
    }

    #[test]
    fn nonempty_str_is_truthy() {
        assert!(Value::str("hi").is_truthy());
    }

    #[test]
    fn empty_array_is_falsy() {
        assert!(!Value::array(vec![]).is_truthy());
    }

    #[test]
    fn nonempty_array_is_truthy() {
        assert!(Value::array(vec![Value::I32(1)]).is_truthy());
    }

    #[test]
    fn record_is_always_truthy() {
        assert!(Value::record(BTreeMap::new()).is_truthy());
    }

    #[test]
    fn tag_is_always_truthy() {
        assert!(Value::tag(0, vec![]).is_truthy());
    }

    // -- Numeric check tests --

    #[test]
    fn is_numeric_true_for_numbers() {
        assert!(Value::I32(0).is_numeric());
        assert!(Value::I64(0).is_numeric());
        assert!(Value::F64(0.0).is_numeric());
    }

    #[test]
    fn is_numeric_false_for_non_numbers() {
        assert!(!Value::Bool(true).is_numeric());
        assert!(!Value::str("hi").is_numeric());
        assert!(!Value::Unit.is_numeric());
        assert!(!Value::array(vec![]).is_numeric());
    }

    // -- Equality tests --

    #[test]
    fn same_values_are_equal() {
        assert_eq!(Value::I32(5), Value::I32(5));
    }

    #[test]
    fn different_values_are_not_equal() {
        assert_ne!(Value::I32(5), Value::I32(6));
    }

    #[test]
    fn cross_type_values_are_not_equal() {
        assert_ne!(Value::I32(5), Value::F64(5.0));
        assert_ne!(Value::I32(0), Value::Bool(false));
        assert_ne!(Value::I32(5), Value::I64(5));
    }

    #[test]
    fn unit_is_only_equal_to_unit() {
        assert_eq!(Value::Unit, Value::Unit);
        assert_ne!(Value::Unit, Value::I32(0));
    }

    #[test]
    fn str_equality_is_deep() {
        assert_eq!(Value::str("hello"), Value::str("hello"));
        assert_ne!(Value::str("hello"), Value::str("world"));
    }

    #[test]
    fn array_equality_is_deep() {
        assert_eq!(
            Value::array(vec![Value::I32(1), Value::I32(2)]),
            Value::array(vec![Value::I32(1), Value::I32(2)]),
        );
        assert_ne!(
            Value::array(vec![Value::I32(1)]),
            Value::array(vec![Value::I32(2)]),
        );
    }

    // -- Debug formatting tests --

    #[test]
    fn debug_i32_shows_number() {
        assert_eq!(format!("{:?}", Value::I32(42)), "42");
    }

    #[test]
    fn debug_i64_shows_suffix() {
        assert_eq!(format!("{:?}", Value::I64(42)), "42i64");
    }

    #[test]
    fn debug_f64_shows_number() {
        assert_eq!(format!("{:?}", Value::F64(2.71)), "2.71");
    }

    #[test]
    fn debug_bool_shows_keyword() {
        assert_eq!(format!("{:?}", Value::Bool(true)), "true");
    }

    #[test]
    fn debug_unit_shows_parens() {
        assert_eq!(format!("{:?}", Value::Unit), "()");
    }

    #[test]
    fn debug_str_shows_quoted() {
        assert_eq!(format!("{:?}", Value::str("hello")), "\"hello\"");
    }

    #[test]
    fn debug_array_shows_bracketed() {
        assert_eq!(
            format!("{:?}", Value::array(vec![Value::I32(1), Value::I32(2)])),
            "[1, 2]",
        );
    }

    #[test]
    fn debug_closure_shows_arity() {
        let data = ClosureData {
            func: FuncPtr::Builtin(Arc::new(Dummy)),
            captures: Arc::from([]),
            arity: 1,
            call_arg_types: Arc::from([]),
        };
        assert_eq!(
            format!("{:?}", Value::Closure(Arc::new(data))),
            "<closure/1>"
        );
    }

    #[test]
    fn debug_effect_shows_label() {
        #[derive(Debug)]
        struct TestEffect;
        impl BuiltinFunction for TestEffect {
            fn name(&self) -> &str {
                "test"
            }
            fn arity(&self) -> usize {
                0
            }
            fn execute(&self, _: &[Value]) -> Result<Value, String> {
                Ok(Value::Unit)
            }
        }
        assert_eq!(
            format!("{:?}", Value::Effect(Arc::new(TestEffect))),
            "<effect>",
        );
    }

    // -- Display tests --

    #[test]
    fn display_str_shows_content() {
        assert_eq!(format!("{}", Value::str("hello")), "hello");
    }

    #[test]
    fn display_non_str_uses_debug() {
        assert_eq!(format!("{}", Value::I32(42)), "42");
    }

    // -- Clone tests --

    #[test]
    fn clone_is_cheap() {
        let v = Value::str("hello");
        let v2 = v.clone();
        assert_eq!(v, v2);
    }

    // -- Edge case: deeply nested array drops cleanly --

    #[test]
    fn deeply_nested_array_drops_without_stack_overflow() {
        let mut v = Value::I32(42);
        for _ in 0..5_000 {
            v = Value::array(vec![v]);
        }
        // If Arc drops recurse too deeply this will stack overflow.
        // 5,000 levels is enough to catch that.
        drop(v);
    }
}
