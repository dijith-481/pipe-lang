use std::collections::HashSet;

use ast::ast::{MatchArm, Pattern};
use ast::span::Span;

use crate::env::TagVariants;
use crate::error::TypeError;
use crate::types::MonoType;

/// Checks that a match expression covers all variants of a tag type.
///
/// When the subject type is a known sum type (e.g. `Option`, `Result`),
/// every variant must have at least one matching arm unless a wildcard
/// or binding pattern covers all remaining cases.
///
/// For non-tag types or unknown tag types, the check is skipped (the
/// caller is responsible for those cases or they are trivially safe).
///
/// Nested pattern exhaustiveness (e.g. `Some(None)` missing `Some(Some(_))`)
/// is **not** checked in this version.
///
/// # Errors
///
/// Returns [`TypeError::NonExhaustiveMatch`] if any variant is uncovered.
pub fn check_exhaustive(
    tag_variants: &TagVariants,
    subj_ty: &MonoType,
    arms: &[MatchArm],
    span: Span,
) -> Result<(), TypeError> {
    if let MonoType::Tag { name, .. } = subj_ty {
        let variants = match tag_variants.get(name) {
            Some(v) => v,
            None => return Ok(()),
        };

        let has_wildcard = arms
            .iter()
            .any(|arm| matches!(arm.pattern, Pattern::Wildcard(_) | Pattern::Binding(_, _)));
        if has_wildcard {
            return Ok(());
        }

        let covered: HashSet<&str> = arms
            .iter()
            .filter_map(|arm| match arm.pattern {
                Pattern::Constructor { name, .. } => Some(*name),
                _ => None,
            })
            .collect();

        let all_covered = variants
            .iter()
            .all(|(vname, _)| covered.contains(vname.as_str()));

        if !all_covered {
            return Err(TypeError::NonExhaustiveMatch { span });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::SmolStr;
    use ast::ast::Expr;
    use ast::span::Span;
    use bumpalo::Bump;
    use std::collections::HashMap;
    use std::rc::Rc;

    fn sp() -> Span {
        Span::new(0, 0)
    }

    fn option_variants() -> TagVariants {
        let mut map = HashMap::new();
        map.insert(
            SmolStr::new("Option"),
            vec![
                (SmolStr::new("None"), vec![]),
                (SmolStr::new("Some"), vec![MonoType::I32]),
            ],
        );
        map
    }

    fn result_variants() -> TagVariants {
        let mut map = HashMap::new();
        map.insert(
            SmolStr::new("Result"),
            vec![
                (SmolStr::new("Ok"), vec![MonoType::I32]),
                (SmolStr::new("Err"), vec![MonoType::Str]),
            ],
        );
        map
    }

    #[test]
    fn all_variants_covered_is_ok() {
        let bump = Bump::new();
        let body = bump.alloc(Expr::int("0", sp(), &bump));
        let arms = vec![
            MatchArm {
                pattern: bump.alloc(Pattern::Constructor {
                    name: "None",
                    fields: bumpalo::collections::Vec::new_in(&bump),
                    span: sp(),
                }),
                body,
            },
            MatchArm {
                pattern: bump.alloc(Pattern::Constructor {
                    name: "Some",
                    fields: bumpalo::collections::Vec::from_iter_in(
                        [Pattern::Wildcard(sp())],
                        &bump,
                    ),
                    span: sp(),
                }),
                body,
            },
        ];
        let result = check_exhaustive(
            &option_variants(),
            &MonoType::Tag {
                name: SmolStr::new("Option"),
                payload: Rc::from([MonoType::I32]),
            },
            &arms,
            sp(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn missing_variant_is_error() {
        let bump = Bump::new();
        let body = bump.alloc(Expr::int("0", sp(), &bump));
        let arms = vec![MatchArm {
            pattern: bump.alloc(Pattern::Constructor {
                name: "Some",
                fields: bumpalo::collections::Vec::from_iter_in([Pattern::Wildcard(sp())], &bump),
                span: sp(),
            }),
            body,
        }];
        let result = check_exhaustive(
            &option_variants(),
            &MonoType::Tag {
                name: SmolStr::new("Option"),
                payload: Rc::from([MonoType::I32]),
            },
            &arms,
            sp(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn wildcard_covers_all() {
        let bump = Bump::new();
        let body = bump.alloc(Expr::int("0", sp(), &bump));
        let arms = vec![MatchArm {
            pattern: bump.alloc(Pattern::Wildcard(sp())),
            body,
        }];
        let result = check_exhaustive(
            &option_variants(),
            &MonoType::Tag {
                name: SmolStr::new("Option"),
                payload: Rc::from([MonoType::I32]),
            },
            &arms,
            sp(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn binding_covers_all() {
        let bump = Bump::new();
        let body = bump.alloc(Expr::int("0", sp(), &bump));
        let arms = vec![MatchArm {
            pattern: bump.alloc(Pattern::Binding("x", sp())),
            body,
        }];
        let result = check_exhaustive(
            &option_variants(),
            &MonoType::Tag {
                name: SmolStr::new("Option"),
                payload: Rc::from([MonoType::I32]),
            },
            &arms,
            sp(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn non_tag_type_skipped() {
        let bump = Bump::new();
        let body = bump.alloc(Expr::int("0", sp(), &bump));
        let arms = vec![MatchArm {
            pattern: bump.alloc(Pattern::Wildcard(sp())),
            body,
        }];
        let result = check_exhaustive(&option_variants(), &MonoType::I32, &arms, sp());
        assert!(result.is_ok());
    }

    #[test]
    fn result_all_variants_covered_is_ok() {
        let bump = Bump::new();
        let body = bump.alloc(Expr::int("0", sp(), &bump));
        let arms = vec![
            MatchArm {
                pattern: bump.alloc(Pattern::Constructor {
                    name: "Ok",
                    fields: bumpalo::collections::Vec::from_iter_in(
                        [Pattern::Wildcard(sp())],
                        &bump,
                    ),
                    span: sp(),
                }),
                body,
            },
            MatchArm {
                pattern: bump.alloc(Pattern::Constructor {
                    name: "Err",
                    fields: bumpalo::collections::Vec::from_iter_in(
                        [Pattern::Wildcard(sp())],
                        &bump,
                    ),
                    span: sp(),
                }),
                body,
            },
        ];
        let result = check_exhaustive(
            &result_variants(),
            &MonoType::Tag {
                name: SmolStr::new("Result"),
                payload: Rc::from([MonoType::I32, MonoType::Str]),
            },
            &arms,
            sp(),
        );
        assert!(result.is_ok());
    }
}
