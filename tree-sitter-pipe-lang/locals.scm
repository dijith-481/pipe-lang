; Scopes
block_expression @local.scope
lambda_expression @local.scope
if_expression consequence: (block_expression) @local.scope
if_expression alternative: (block_expression) @local.scope

; Definitions
let_declaration name: (identifier) @local.definition
let_statement pattern: (binding_pattern) @local.definition
lambda_expression (param name: (identifier) @local.definition)
match_arm pattern: (binding_pattern) @local.definition

; References
identifier @local.reference
