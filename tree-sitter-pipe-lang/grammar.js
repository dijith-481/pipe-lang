module.exports = grammar({
    name: 'pipe_lang',

    extras: $ => [/\s/, $.comment],

    conflicts: $ => [
        [$.expression_statement, $.index_expression],
        [$.expression_statement, $.field_access_expression],
        [$.expression_statement, $.application_expression],
        [$.expression_statement, $.binary_expression],
        [$.expression_statement, $.unary_expression],
        [$.type_expression, $.identifier],
        [$.expression, $.param],
        [$.block_expression, $.record_expression],
        [$.lambda_expression, $.index_expression],
        [$.lambda_expression, $.field_access_expression],
        [$.lambda_expression, $.application_expression],
        [$.lambda_expression, $.binary_expression],
        [$.lambda_expression, $.unary_expression],
        [$.let_statement, $.binary_expression],
        [$.let_statement, $.unary_expression],
        [$.let_statement, $.index_expression],
        [$.let_statement, $.field_access_expression],
        [$.let_statement, $.application_expression],
    ],
    // Expression-internal conflicts: postfix ops bind tighter
    // than statement boundaries inside blocks.
    // Tree-sitter's GLR resolves these by preferring the longer match.

    rules: {
        source_file: $ => repeat($.declaration),

        declaration: $ => choice(
            $.let_declaration,
            $.type_declaration,
            $.use_declaration,
        ),

        let_declaration: $ => seq(
            'let',
            field('name', $.identifier),
            optional(seq(':', field('type', $.type_expression))),
            '=',
            prec(1, field('value', $.expression)),
        ),

        type_declaration: $ => seq(
            'type',
            field('name', $.identifier),
            optional(seq('<', commaSep1($.type_parameter), '>')),
            '=',
            field('rhs', $.type_expression),
        ),

        use_declaration: $ => seq(
            'use',
            commaSep1($.identifier),
        ),

        type_parameter: $ => $.identifier,

        type_expression: $ => choice(
            $.type_application,
            $.type_arrow,
            $.type_record,
            $.type_array,
            $.type_unit,
            $.identifier,
        ),

        type_application: $ => prec(1, seq(
            field('constructor', $.identifier),
            '<',
            commaSep1($.type_expression),
            '>',
        )),

        type_arrow: $ => prec.right(-1, seq(
            field('from', $.type_expression),
            '->',
            field('to', $.type_expression),
        )),

        type_record: $ => seq(
            '{',
            commaSep(seq(
                field('field_name', $.identifier),
                ':',
                field('field_type', $.type_expression),
            )),
            '}',
        ),

        type_array: $ => seq('[', field('element', $.type_expression), ']'),

        type_unit: $ => seq('(', ')'),

        expression: $ => choice(
            $.binary_expression,
            $.unary_expression,
            $.application_expression,
            $.if_expression,
            $.match_expression,
            $.block_expression,
            $.lambda_expression,
            $.record_expression,
            $.array_expression,
            $.tuple_expression,
            $.field_access_expression,
            $.index_expression,
            $.template_expression,
            $.literal,
            $.identifier,
        ),

        application_expression: $ => prec(2, seq(
            field('function', $.expression),
            '(',
            commaSep(field('argument', $.expression)),
            ')',
        )),

        binary_expression: $ => choice(
            ...[
                ['+', 1],
                ['-', 1],
                ['*', 2],
                ['/', 2],
                ['%', 2],
                ['==', -1],
                ['!=', -1],
                ['<', -1],
                ['<=', -1],
                ['>', -1],
                ['>=', -1],
                ['&&', -2],
                ['||', -3],
            ].map(([op, prec_level]) =>
                prec.left(prec_level, seq(
                    field('left', $.expression),
                    op,
                    field('right', $.expression),
                ))
            ),
        ),

        unary_expression: $ => prec(3, seq(
            field('operator', choice('-', '!')),
            field('operand', $.expression),
        )),

        if_expression: $ => seq(
            'if',
            field('condition', $.expression),
            field('consequence', $.block_expression),
            optional(seq(
                'else',
                field('alternative', choice($.if_expression, $.block_expression)),
            )),
        ),

        match_expression: $ => seq(
            'match',
            field('subject', $.expression),
            '{',
            repeat($.match_arm),
            '}',
        ),

        match_arm: $ => seq(
            field('pattern', $.pattern),
            '=>',
            field('value', $.expression),
        ),

        block_expression: $ => seq(
            '{',
            repeat($.statement),
            '}',
        ),

        statement: $ => choice(
            $.let_statement,
            $.expression_statement,
        ),

        let_statement: $ => seq(
            'let',
            field('pattern', $.pattern),
            '=',
            prec(1, field('value', $.expression)),
        ),

        expression_statement: $ => $.expression,

        lambda_expression: $ => seq(
            '(',
            commaSep($.param),
            ')',
            '=>',
            field('body', $.expression),
        ),

        param: $ => seq(
            field('name', $.identifier),
            optional(seq(':', field('type', $.type_expression))),
        ),

        record_expression: $ => seq(
            '{',
            commaSep(seq(
                field('key', $.identifier),
                ':',
                field('value', $.expression),
            )),
            '}',
        ),

        array_expression: $ => seq(
            '[',
            commaSep(field('element', $.expression)),
            ']',
        ),

        tuple_expression: $ => seq(
            '(',
            commaSep1(field('element', $.expression)),
            ')',
        ),

        field_access_expression: $ => seq(
            field('object', $.expression),
            '.',
            field('field', $.identifier),
        ),

        index_expression: $ => seq(
            field('object', $.expression),
            '[',
            field('index', $.expression),
            ']',
        ),

        template_expression: $ => seq(
            '`',
            repeat(choice(
                $.template_string,
                $.template_interpolation,
            )),
            '`',
        ),

        template_string: $ => token(prec(1, /[^`$\\]+/)),

        template_interpolation: $ => seq(
            '${',
            field('value', $.expression),
            '}',
        ),

        literal: $ => choice(
            $.integer_literal,
            $.float_literal,
            $.string_literal,
            $.boolean_literal,
        ),

        integer_literal: $ => /[0-9][_0-9]*/,

        float_literal: $ => /[0-9][_0-9]*\.[0-9][_0-9]*/,

        string_literal: $ => seq('"', repeat(choice(/[^"\\]+/, /\\./)), '"'),

        boolean_literal: $ => choice('true', 'false'),

        comment: $ => token(seq('//', /[^\n]*/)),

        pattern: $ => choice(
            $.wildcard_pattern,
            $.binding_pattern,
            $.literal_pattern,
            $.constructor_pattern,
            $.tuple_pattern,
            $.record_pattern,
        ),

        wildcard_pattern: $ => '_',

        binding_pattern: $ => $.identifier,

        literal_pattern: $ => $.literal,

        constructor_pattern: $ => seq(
            field('name', $.identifier),
            '(', commaSep(field('argument', $.pattern)), ')',
        ),

        tuple_pattern: $ => seq(
            '(',
            commaSep1(field('element', $.pattern)),
            ')',
        ),

        record_pattern: $ => seq(
            '{',
            commaSep(seq(
                field('field', $.identifier),
                optional(seq(':', field('pattern', $.pattern))),
            )),
            '}',
        ),

        identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,
    },
});

function commaSep(rule) {
    return optional(commaSep1(rule));
}

function commaSep1(rule) {
    return seq(rule, repeat(seq(',', rule)));
}
