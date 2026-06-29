use std::ffi::CStr;
use std::os::raw::c_char;

/// Returns the tree-sitter parser for pipe-lang.
///
/// # Safety
///
/// The returned pointer must be freed with `tree_sitter_parser_destroy`.
#[no_mangle]
pub extern "C" fn tree_sitter_pipe_lang() -> *mut tree_sitter::Parser {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_language())
        .expect("Failed to set pipe-lang grammar");
    Box::into_raw(Box::new(parser))
}

/// Returns the tree-sitter language object for pipe-lang.
#[no_mangle]
pub extern "C" fn tree_sitter_language() -> tree_sitter::Language {
    extern "C" {
        fn tree_sitter_pipe_lang_raw() -> *const tree_sitter::LanguageFn;
    }
    unsafe { tree_sitter::Language::from_raw(tree_sitter_pipe_lang_raw()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_parse_hello() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_language())
            .expect("Failed to set pipe-lang grammar");

        let source = "let main = () => println(`Hello, World!`)";
        let tree = parser.parse(source, None).expect("Failed to parse");
        let root = tree.root_node();

        assert_eq!(root.kind(), "source_file");
        assert_eq!(root.child_count(), 1);
        assert_eq!(root.child(0).unwrap().kind(), "let_binding");
    }

    #[test]
    fn test_can_parse_types() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_language())
            .expect("Failed to set pipe-lang grammar");

        let source = r#"
type Option<T> =
    | Some(T)
    | None
"#;
        let tree = parser.parse(source, None).expect("Failed to parse");
        let root = tree.root_node();
        assert_eq!(root.kind(), "source_file");
    }
}
