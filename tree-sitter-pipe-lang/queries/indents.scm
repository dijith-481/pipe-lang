; Indentation rules for tree-sitter indentation
; Blocks increase indent
(block_expression "{" @indent "}" @indent)
(match_block "{" @indent "}" @indent)

; Record literals increase indent
(record_literal "{" @indent "}" @indent)
(type_record "{" @indent "}" @indent)

; Arrays increase indent
(array_literal "[" @indent "]" @indent)

; Match arms typically indent
(match_arm "=>" @indent)

; Continuation indent for binary expressions split across lines
(binary_expression) @indent
