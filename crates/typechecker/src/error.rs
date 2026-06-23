use ast::span::Span;
use diagnostics::CompilerError;

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

impl From<TypeError> for CompilerError {
    fn from(err: TypeError) -> Self {
        match err {
            TypeError::UnificationFailed {
                expected,
                got,
                span,
            } => CompilerError::type_error(
                span,
                format!("type mismatch: expected {expected}, got {got}"),
            ),
            TypeError::UnboundVariable { name, span } => {
                CompilerError::unbound_variable(span, name)
            }
            TypeError::ArityMismatch {
                expected,
                got,
                span,
            } => CompilerError::type_error(
                span,
                format!("arity mismatch: expected {expected} arguments, got {got}"),
            ),
            TypeError::InfiniteType { var, ty, span } => {
                CompilerError::type_error(span, format!("infinite type: {var} occurs in {ty}"))
            }
            TypeError::AnnotationConflict {
                annotation,
                inferred,
                span,
            } => {
                let msg = format!(
                    "type annotation conflict: annotation says {annotation}, inferred {inferred}"
                );
                CompilerError::type_error(span, msg)
            }
            TypeError::NonExhaustiveMatch { span } => CompilerError::non_exhaustive_match(span),
            TypeError::FieldNotFound { field, span } => {
                CompilerError::type_error(span, format!("field `{field}` not found on record"))
            }
            TypeError::NumericOverflow { ty, span } => {
                CompilerError::type_error(span, format!("numeric literal overflows type `{ty}`"))
            }
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
