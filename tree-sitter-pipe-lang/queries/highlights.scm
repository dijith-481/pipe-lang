; ---------------------------------------------------------------------------
; Syntax highlighting queries for pipe-lang
; ---------------------------------------------------------------------------

; --- Comments ---
(comment) @comment

; --- Keywords ---
[
  "let"
  "type"
  "match"
  "if"
  "else"
  "use"
] @keyword

; --- Conditionals ---
(if_expression "if" @conditional)
(if_expression "else" @conditional)

; --- Match ---
(match_expression "match" @keyword)

; --- Literals ---
(integer_literal) @number
(float_literal) @number.float
(string_literal) @string
(template_literal) @string.special
(boolean_literal) @boolean

; --- Operators ---
(binary_expression operator: _ @operator)
(unary_expression [
  "-"
  "!"
] @operator)

; --- Delimiters ---
[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  "."
  ","
  ";"
  ":"
  "::"
] @punctuation.delimiter

; --- Lambda arrow ---
(lambda_expression "=>" @operator)

; --- Type arrow ---
(type_function "->" @operator)

; --- Identifiers ---
(identifier) @variable

; -- Parameter names --
(lambda_param name: (identifier) @parameter)

; -- Type names (in type definitions) --
(type_alias name: (identifier) @type.definition)
(type_variant name: (identifier) @type)

; -- Type references --
(type_named) @type

; -- Function names (let bindings) --
(let_binding name: (identifier) @function)

; -- Fields --
(record_field name: (identifier) @property)
(record_pattern_field name: (identifier) @property)

; -- Pattern matching --
(wildcard_pattern) @constant.builtin
(constructor_pattern name: (identifier) @type)

; --- Type parameters ---
(type_parameters (identifier) @type.parameter)

; --- Path segments ---
(path (identifier) @module)

; --- Pipes (sum types) ---
(type_sum "|" @punctuation.delimiter)
