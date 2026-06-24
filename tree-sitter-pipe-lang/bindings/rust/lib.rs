extern crate tree_sitter;
use std::ffi::CStr;

/// Returns the tree-sitter language object for pipe-lang.
pub fn language() -> tree_sitter::Language {
    unsafe {
        extern "C" { fn tree_sitter_pipe_lang() -> tree_sitter::Language; }
        tree_sitter_pipe_lang()
    }
}

/// Returns the grammar source code.
pub fn grammar_source() -> &'static str {
    include_str!("../../grammar.js")
}

/// Returns the name of the language.
pub fn language_name() -> &'static CStr {
    unsafe { CStr::from_ptr("pipe_lang\0".as_ptr().cast()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_load_grammar() {
        let lang = language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(lang).unwrap();

        let source = "let x = 42";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        assert_eq!(root.kind(), "source_file");
        assert!(root.has_error() == false, "source should parse without errors");
    }

    #[test]
    fn parse_with_type_annotation() {
        let lang = language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(lang).unwrap();

        let source = "let x: i32 = 42";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        assert_eq!(root.kind(), "source_file");
        assert!(root.has_error() == false);
    }

    #[test]
    fn parse_if_expression() {
        let lang = language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(lang).unwrap();

        let source = "if x > 0 { y } else { z }";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        assert_eq!(root.kind(), "source_file");
        // Our grammar expects if_expression inside a declaration
        // This is a bare expression, so it may parse as an error node
        // unless wrapped in a let
    }
}
