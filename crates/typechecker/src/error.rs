use ast::span::Span;

use crate::types::{MonoType, TypeId};

/// Errors produced by the type checker.
#[derive(Debug, Clone, thiserror::Error)]
pub enum TypeError {
    /// Two types that should unify failed to match.
    #[error("type mismatch: expected {expected}, got {got}")]
    UnificationFailed {
        expected: MonoType,
        got: MonoType,
        span: Span,
    },

    /// A variable was used but not bound in the type environment.
    #[error("unbound variable `{name}`")]
    UnboundVariable { name: String, span: Span },

    /// A function was called with the wrong number of arguments.
    #[error("arity mismatch: expected {expected} arguments, got {got}")]
    ArityMismatch {
        expected: usize,
        got: usize,
        span: Span,
    },

    /// An infinite type was detected (occurs check failure).
    #[error("infinite type: {var} occurs in {ty}")]
    InfiniteType {
        var: TypeId,
        ty: MonoType,
        span: Span,
    },

    /// A type annotation conflicts with the inferred type.
    #[error("type annotation conflict: annotation says {annotation}, inferred {inferred}")]
    AnnotationConflict {
        annotation: MonoType,
        inferred: MonoType,
        span: Span,
    },

    /// Pattern match is not exhaustive.
    #[error("non-exhaustive match: missing patterns")]
    NonExhaustiveMatch { span: Span },

    /// A field was not found on a record type.
    #[error("field `{field}` not found on record")]
    FieldNotFound { field: String, span: Span },

    /// A numeric literal overflowed its type.
    #[error("numeric literal overflows type `{ty}`")]
    NumericOverflow { ty: MonoType, span: Span },
}

impl TypeError {
    /// Returns the source span for this error.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            TypeError::UnificationFailed { span, .. }
            | TypeError::UnboundVariable { span, .. }
            | TypeError::ArityMismatch { span, .. }
            | TypeError::InfiniteType { span, .. }
            | TypeError::AnnotationConflict { span, .. }
            | TypeError::NonExhaustiveMatch { span }
            | TypeError::FieldNotFound { span, .. }
            | TypeError::NumericOverflow { span, .. } => *span,
        }
    }
}

impl From<TypeError> for diagnostics::CompilerError {
    fn from(err: TypeError) -> Self {
        match err {
            TypeError::UnificationFailed {
                span,
                expected,
                got,
            } => diagnostics::CompilerError::type_mismatch(
                expected.to_string(),
                got.to_string(),
                span,
            ),
            TypeError::UnboundVariable { name, span } => {
                diagnostics::CompilerError::unbound_variable(name, span)
            }
            TypeError::ArityMismatch {
                expected,
                got,
                span,
            } => diagnostics::CompilerError::type_mismatch(
                format!("{expected} arguments"),
                format!("{got} arguments"),
                span,
            ),
            TypeError::InfiniteType { var, ty, span } => diagnostics::CompilerError::TypeMismatch {
                expected: "finite type".to_string(),
                got: format!("Type var {var} occurs in {ty}"),
                span,
            },
            TypeError::AnnotationConflict {
                annotation,
                inferred,
                span,
            } => diagnostics::CompilerError::type_mismatch(
                annotation.to_string(),
                inferred.to_string(),
                span,
            ),
            TypeError::NonExhaustiveMatch { span } => {
                diagnostics::CompilerError::non_exhaustive_match(span)
            }
            TypeError::FieldNotFound { field, span } => diagnostics::CompilerError::type_mismatch(
                "record with field".to_string(),
                field,
                span,
            ),
            TypeError::NumericOverflow { ty, span } => diagnostics::CompilerError::TypeMismatch {
                expected: format!("value within range of {ty}"),
                got: "overflow".to_string(),
                span,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unification_failed_display() {
        let err = TypeError::UnificationFailed {
            expected: MonoType::I32,
            got: MonoType::Str,
            span: Span::new(5, 10),
        };
        assert!(format!("{err}").contains("type mismatch"));
    }

    #[test]
    fn unbound_variable_display() {
        let err = TypeError::UnboundVariable {
            name: "x".into(),
            span: Span::new(0, 1),
        };
        assert!(format!("{err}").contains("unbound variable"));
        assert!(format!("{err}").contains("x"));
    }

    #[test]
    fn error_span_extraction() {
        let err = TypeError::ArityMismatch {
            expected: 2,
            got: 3,
            span: Span::new(10, 15),
        };
        assert_eq!(err.span(), Span::new(10, 15));
    }
}
