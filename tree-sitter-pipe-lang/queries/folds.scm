; Fold points
(block_expression "{" @fold "}" @fold)
(match_block "{" @fold "}" @fold)
(array_literal "[" @fold "]" @fold)
(record_literal "{" @fold "}" @fold)
(type_record "{" @fold "}" @fold)
(parenthesized_expression "(" @fold ")" @fold)
(lambda_param_list "(" @fold ")" @fold)
(type_tuple "(" @fold ")" @fold)
