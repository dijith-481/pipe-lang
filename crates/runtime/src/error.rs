use std::fmt;

/// Errors that can occur during runtime execution.
#[derive(Debug, Clone)]
pub enum RuntimeError {
    /// A value was used where a different type was expected.
    TypeMismatch { expected: String, got: String },

    /// An argument count mismatch (wrong arity).
    ArityMismatch { expected: usize, got: usize },

    /// Division by zero.
    DivisionByZero,

    /// Index out of bounds for an array.
    IndexOutOfBounds { index: i64, len: usize },

    /// Field not found on a record.
    FieldNotFound { field: String },

    /// Variable not found in scope.
    UnboundVariable { name: String },

    /// Pattern match exhaustiveness failure.
    NonExhaustiveMatch { value: String },

    /// An effect could not be executed.
    EffectError { msg: String },

    /// A user-thrown error from the language.
    UserError { msg: String },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::TypeMismatch { expected, got } => {
                write!(f, "type mismatch: expected {expected}, got {got}")
            }
            RuntimeError::ArityMismatch { expected, got } => {
                write!(
                    f,
                    "arity mismatch: expected {expected} arguments, got {got}"
                )
            }
            RuntimeError::DivisionByZero => write!(f, "division by zero"),
            RuntimeError::IndexOutOfBounds { index, len } => {
                write!(f, "index {index} out of bounds for array of length {len}")
            }
            RuntimeError::FieldNotFound { field } => {
                write!(f, "field `{field}` not found")
            }
            RuntimeError::UnboundVariable { name } => {
                write!(f, "unbound variable `{name}`")
            }
            RuntimeError::NonExhaustiveMatch { value } => {
                write!(f, "non-exhaustive match: no arm handles {value}")
            }
            RuntimeError::EffectError { msg } => {
                write!(f, "effect error: {msg}")
            }
            RuntimeError::UserError { msg } => {
                write!(f, "{msg}")
            }
        }
    }
}

impl std::error::Error for RuntimeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_mismatch_display() {
        let err = RuntimeError::TypeMismatch {
            expected: "Int".into(),
            got: "Str".into(),
        };
        assert_eq!(format!("{err}"), "type mismatch: expected Int, got Str");
    }

    #[test]
    fn arity_mismatch_display() {
        let err = RuntimeError::ArityMismatch {
            expected: 2,
            got: 3,
        };
        assert_eq!(
            format!("{err}"),
            "arity mismatch: expected 2 arguments, got 3"
        );
    }

    #[test]
    fn division_by_zero_display() {
        let err = RuntimeError::DivisionByZero;
        assert_eq!(format!("{err}"), "division by zero");
    }

    #[test]
    fn index_out_of_bounds_display() {
        let err = RuntimeError::IndexOutOfBounds { index: 10, len: 5 };
        assert_eq!(
            format!("{err}"),
            "index 10 out of bounds for array of length 5"
        );
    }

    #[test]
    fn field_not_found_display() {
        let err = RuntimeError::FieldNotFound {
            field: "age".into(),
        };
        assert_eq!(format!("{err}"), "field `age` not found");
    }

    #[test]
    fn unbound_variable_display() {
        let err = RuntimeError::UnboundVariable { name: "x".into() };
        assert_eq!(format!("{err}"), "unbound variable `x`");
    }

    #[test]
    fn runtime_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(RuntimeError::DivisionByZero);
        assert_eq!(format!("{err}"), "division by zero");
    }
}
