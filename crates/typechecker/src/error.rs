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
            } => diagnostics::CompilerError::type_error(
                span,
                format!("type mismatch: expected `{expected}`, got `{got}`"),
            ),
            TypeError::UnboundVariable { name, span } => {
                diagnostics::CompilerError::type_error(
                    span,
                    format!(
                        "unbound variable `{name}` — make sure it is spelled \
                         correctly and in scope"
                    ),
                )
            }
            TypeError::ArityMismatch {
                expected,
                got,
                span,
            } => diagnostics::CompilerError::type_error(
                span,
                format!(
                    "arity mismatch: this function expects {expected} argument(s), \
                     but {got} were provided"
                ),
            ),
            TypeError::InfiniteType { var: _var, ty, span } => {
                diagnostics::CompilerError::type_error(
                    span,
                    format!(
                        "recursive type constraint — `{ty}` references itself. \
                         Try adding a type annotation"
                    ),
                )
            }
            TypeError::AnnotationConflict {
                annotation,
                inferred,
                span,
            } => diagnostics::CompilerError::type_error(
                span,
                format!(
                    "type annotation says `{annotation}`, \
                     but the expression is inferred as `{inferred}`"
                ),
            ),
            TypeError::NonExhaustiveMatch { span } => {
                diagnostics::CompilerError::type_error(
                    span,
                    "non-exhaustive match — add a wildcard pattern `_` to \
                     catch all unmatched cases",
                )
            }
            TypeError::FieldNotFound { field, span } => {
                diagnostics::CompilerError::type_error(
                    span,
                    format!("field `{field}` not found on this record type"),
                )
            }
            TypeError::NumericOverflow { ty, span } => diagnostics::CompilerError::type_error(
                span,
                format!(
                    "numeric literal overflows `{ty}` — use a larger type \
                     like `i64` or `f64`"
                ),
            ),
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
