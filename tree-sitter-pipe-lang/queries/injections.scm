; Template literal injections for embedded expressions
; Currently handled within the template_literal token itself.
; For proper injection (${expr}), the grammar tokenizes the whole template
; as one token. Editors that support tree-sitter injections can use:
; (template_literal) @injection.content
;   (#set! injection.language "pipe_lang")
