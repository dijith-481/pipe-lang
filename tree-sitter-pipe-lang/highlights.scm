; Keywords
"let" @keyword
"type" @keyword
"use" @keyword
"if" @keyword
"else" @keyword
"match" @keyword
"true" @boolean
"false" @boolean
"_" @character.special

; Types
type_expression @type
type_parameter @type.parameter
(param type: (type_expression) @type)

; Function calls
application_expression function: (identifier) @function

; Built-in types
((identifier) @type.builtin
  (#eq? @type.builtin "i32"))
((identifier) @type.builtin
  (#eq? @type.builtin "i64"))
((identifier) @type.builtin
  (#eq? @type.builtin "f32"))
((identifier) @type.builtin
  (#eq? @type.builtin "f64"))
((identifier) @type.builtin
  (#eq? @type.builtin "str"))
((identifier) @type.builtin
  (#eq? @type.builtin "bool"))

; Identifiers
identifier @variable
let_declaration name: (identifier) @function
param name: (identifier) @variable.parameter

; Literals
integer_literal @number
float_literal @number.float
string_literal @string
boolean_literal @boolean

; Operators
"+" @operator
"-" @operator
"*" @operator
"/" @operator
"%" @operator
"==" @operator
"!=" @operator
"<" @operator
"<=" @operator
">" @operator
">=" @operator
"&&" @operator
"||" @operator
"!" @operator
"->" @operator
"=>" @operator
"=" @operator
":" @operator
"." @operator

; Delimiters
"(" @punctuation.bracket
")" @punctuation.bracket
"{" @punctuation.bracket
"}" @punctuation.bracket
"[" @punctuation.bracket
"]" @punctuation.bracket
"`" @punctuation.bracket
"${" @punctuation.special
"," @punctuation.delimiter

; Comments
comment @comment @spell

; Templates
template_string @string
template_interpolation @embedded
