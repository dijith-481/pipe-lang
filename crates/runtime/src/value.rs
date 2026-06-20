use std::collections::BTreeMap;
use std::sync::Arc;

use ast::SmolStr;

/// A runtime value shared by the interpreter-facing standard library and JIT.
///
/// The enum is intentionally small: it mirrors the value kinds currently
/// emitted by lowering, which keeps JIT layout handling tractable.
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A 32-bit signed integer.
    I32(i32),
    /// A 64-bit signed integer.
    I64(i64),
    /// A 64-bit floating point number.
    F64(f64),
    /// A boolean.
    Bool(bool),
    /// The unit value, `()`.
    Unit,
    /// A reference-counted UTF-8 string slice.
    Str(Arc<str>),
    /// A reference-counted immutable array of runtime values.
    Array(Arc<[Value]>),
    /// A reference-counted record value.
    Record(Arc<RecordData>),
    /// A reference-counted closure value.
    Closure(Arc<ClosureData>),
    /// A sum-type value with a numeric discriminant and payload.
    Tag { tag: u32, payload: Arc<[Value]> },
}

impl Value {
    /// Creates a string runtime value.
    ///
    /// # Arguments
    ///
    /// * `s` - The string data to store behind an [`Arc<str>`].
    ///
    /// # Returns
    ///
    /// A [`Value::Str`] containing the provided text.
    #[must_use]
    pub fn str(s: impl Into<Arc<str>>) -> Self {
        Self::Str(s.into())
    }

    /// Creates an immutable array runtime value.
    ///
    /// # Arguments
    ///
    /// * `values` - The values to move into the array.
    ///
    /// # Returns
    ///
    /// A [`Value::Array`] backed by an [`Arc<[Value]>`].
    #[must_use]
    pub fn array(values: Vec<Value>) -> Self {
        Self::Array(Arc::from(values.into_boxed_slice()))
    }

    /// Creates a record runtime value.
    ///
    /// # Arguments
    ///
    /// * `fields` - Field names mapped to their runtime values.
    ///
    /// # Returns
    ///
    /// A [`Value::Record`] backed by [`RecordData`].
    #[must_use]
    pub fn record(fields: BTreeMap<SmolStr, Value>) -> Self {
        Self::Record(Arc::new(RecordData { fields }))
    }

    /// Creates a tagged-union runtime value.
    ///
    /// # Arguments
    ///
    /// * `tag` - The numeric discriminant for the variant.
    /// * `payload` - Values stored by the variant.
    ///
    /// # Returns
    ///
    /// A [`Value::Tag`] with immutable payload storage.
    #[must_use]
    pub fn tag(tag: u32, payload: Vec<Value>) -> Self {
        Self::Tag {
            tag,
            payload: Arc::from(payload.into_boxed_slice()),
        }
    }

    /// Returns the contained `i32`, if this is [`Value::I32`].
    #[must_use]
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Self::I32(value) => Some(*value),
            Self::I64(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::Unit
            | Self::Str(_)
            | Self::Array(_)
            | Self::Record(_)
            | Self::Closure(_)
            | Self::Tag { .. } => None,
        }
    }

    /// Returns the contained `i64`, if this is [`Value::I64`].
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::I64(value) => Some(*value),
            Self::I32(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::Unit
            | Self::Str(_)
            | Self::Array(_)
            | Self::Record(_)
            | Self::Closure(_)
            | Self::Tag { .. } => None,
        }
    }

    /// Returns the contained `f64`, if this is [`Value::F64`].
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::F64(value) => Some(*value),
            Self::I32(_)
            | Self::I64(_)
            | Self::Bool(_)
            | Self::Unit
            | Self::Str(_)
            | Self::Array(_)
            | Self::Record(_)
            | Self::Closure(_)
            | Self::Tag { .. } => None,
        }
    }

    /// Returns the contained `bool`, if this is [`Value::Bool`].
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            Self::I32(_)
            | Self::I64(_)
            | Self::F64(_)
            | Self::Unit
            | Self::Str(_)
            | Self::Array(_)
            | Self::Record(_)
            | Self::Closure(_)
            | Self::Tag { .. } => None,
        }
    }

    /// Returns the contained string slice, if this is [`Value::Str`].
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(value) => Some(value),
            Self::I32(_)
            | Self::I64(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::Unit
            | Self::Array(_)
            | Self::Record(_)
            | Self::Closure(_)
            | Self::Tag { .. } => None,
        }
    }

    /// Returns the contained array slice, if this is [`Value::Array`].
    #[must_use]
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(values) => Some(values),
            Self::I32(_)
            | Self::I64(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::Unit
            | Self::Str(_)
            | Self::Record(_)
            | Self::Closure(_)
            | Self::Tag { .. } => None,
        }
    }
}

/// Data stored by a record value.
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecordData {
    /// Record fields keyed by source-level field name.
    pub fields: BTreeMap<SmolStr, Value>,
}

/// Data stored by a closure value.
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct ClosureData {
    /// Address of the compiled function entry point.
    pub func_ptr: usize,
    /// Values captured by the closure environment.
    pub captures: Arc<[Value]>,
    /// Number of arguments expected when the closure is called.
    pub arity: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_str_constructor_stores_text() {
        let value = Value::str("hello");

        assert_eq!(value.as_str(), Some("hello"));
    }

    #[test]
    fn value_array_constructor_stores_values() {
        let value = Value::array(vec![Value::I32(1), Value::I32(2)]);

        assert_eq!(value.as_array(), Some([Value::I32(1), Value::I32(2)].as_slice()));
    }

    #[test]
    fn value_tag_constructor_stores_tag_and_payload() {
        let value = Value::tag(1, vec![Value::Bool(true)]);

        match value {
            Value::Tag { tag, payload } => {
                assert_eq!(tag, 1);
                assert_eq!(payload.as_ref(), [Value::Bool(true)].as_slice());
            }
            actual => panic!("expected tag, got {actual:?}"),
        }
    }

    #[test]
    fn value_record_constructor_stores_fields() {
        let mut fields = BTreeMap::new();
        fields.insert(SmolStr::new("answer"), Value::I32(42));
        let value = Value::record(fields);

        match value {
            Value::Record(record) => {
                assert_eq!(record.fields.get("answer"), Some(&Value::I32(42)));
            }
            actual => panic!("expected record, got {actual:?}"),
        }
    }

    #[test]
    fn primitive_accessors_match_kept_types() {
        assert_eq!(Value::I32(1).as_i32(), Some(1));
        assert_eq!(Value::I64(2).as_i64(), Some(2));
        assert_eq!(Value::F64(3.5).as_f64(), Some(3.5));
        assert_eq!(Value::Bool(false).as_bool(), Some(false));
        assert_eq!(Value::Unit.as_i32(), None);
    }
}
