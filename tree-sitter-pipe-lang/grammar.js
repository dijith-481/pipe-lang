/// <reference types="tree-sitter-cli" />

module.exports = grammar({
  name: 'pipe_lang',

  extras: $ => [/\s/, $.comment],

  conflicts: $ => [
    [$.record_literal, $.block_expression],
    [$.block_expression, $._statement],
    [$.type_tuple, $.parenthesized_type],
    [$.lambda_param, $._expression],
  ],

  word: $ => $.identifier,

  rules: {
    // -----------------------------------------------------------------------
    // Source file
    // -----------------------------------------------------------------------
    source_file: $ => repeat($._declaration),

    _declaration: $ => choice(
      $.let_binding,
      $.type_alias,
      $.use_declaration,
    ),

    // -----------------------------------------------------------------------
    // Comments
    // -----------------------------------------------------------------------
    comment: $ => token(seq('//', /.*/)),

    // -----------------------------------------------------------------------
    // Identifiers & literals
    // -----------------------------------------------------------------------
    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    integer_literal: $ => token(choice(
      seq(/[0-9][0-9_]*/, optional(choice('i8', 'i16', 'i32', 'i64', 'u8', 'u16', 'u32', 'u64', 'usize'))),
      seq('0x', /[0-9a-fA-F][0-9a-fA-F_]*/, optional(choice('i8', 'i16', 'i32', 'i64', 'u8', 'u16', 'u32', 'u64', 'usize'))),
      seq('0o', /[0-7][0-7_]*/, optional(choice('i8', 'i16', 'i32', 'i64', 'u8', 'u16', 'u32', 'u64', 'usize'))),
      seq('0b', /[01][01_]*/, optional(choice('i8', 'i16', 'i32', 'i64', 'u8', 'u16', 'u32', 'u64', 'usize'))),
    )),

    float_literal: $ => token(seq(
      /[0-9][0-9_]*/, '.', /[0-9][0-9_]*/, optional(choice('f32', 'f64')),
    )),

    string_literal: $ => token(seq(
      '"',
      repeat(choice(
        token.immediate(/[^"\\]/),
        token.immediate(seq('\\', /./)),
      )),
      '"',
    )),

    boolean_literal: $ => choice('true', 'false'),

    // -----------------------------------------------------------------------
    // Template literals: `Hello, ${name}!`
    // -----------------------------------------------------------------------
    template_literal: $ => token(seq(
      '`',
      repeat(choice(
        token.immediate(/[^`$\\]/),
        token.immediate(seq('\\', /./)),
        token.immediate(seq('${', /[^}]*/, '}')),
        token.immediate(/\$/),
      )),
      '`',
    )),

    // -----------------------------------------------------------------------
    // Keywords
    // -----------------------------------------------------------------------
    _keyword: $ => choice('let', 'type', 'match', 'if', 'else', 'true', 'false', 'use'),

    // -----------------------------------------------------------------------
    // Declarations
    // -----------------------------------------------------------------------
    let_binding: $ => seq(
      'let',
      field('name', $._pattern),
      optional(seq(':', field('type_annotation', $._type_expression))),
      '=',
      field('value', $._expression),
    ),

    type_alias: $ => seq(
      'type',
      field('name', $.identifier),
      optional($.type_parameters),
      '=',
      field('type', $._type_expression),
    ),

    type_parameters: $ => seq(
      '<',
      $.identifier,
      repeat(seq(',', $.identifier)),
      optional(','),
      '>',
    ),

    use_declaration: $ => seq(
      'use',
      $.path,
    ),

    path: $ => seq(
      $.identifier,
      repeat(seq('::', $.identifier)),
    ),

    // -----------------------------------------------------------------------
    // Type expressions
    // -----------------------------------------------------------------------
    _type_expression: $ => choice(
      $.type_sum,
      $.type_function,
      $.type_apply,
      $.type_named,
      $.type_tuple,
      $.type_record,
      $.parenthesized_type,
    ),

    type_named: $ => prec(2, $.identifier),

    type_apply: $ => prec(1, seq(
      field('func', $._type_expression),
      '<',
      field('arg', $._type_expression),
      repeat(seq(',', field('arg', $._type_expression))),
      optional(','),
      '>',
    )),

    type_function: $ => prec.right(0, seq(
      field('from', choice(
        $.type_tuple,
        $.type_named,
        $.type_apply,
        $.type_record,
        $.parenthesized_type,
      )),
      '->',
      field('to', $._type_expression),
    )),

    type_tuple: $ => seq(
      '(',
      $._type_expression,
      repeat(seq(',', $._type_expression)),
      optional(','),
      ')',
    ),

    type_record: $ => seq(
      '{',
      repeat(seq(
        $.type_record_field,
        optional(','),
      )),
      '}',
    ),

    type_record_field: $ => seq(
      field('name', $.identifier),
      ':',
      field('type', $._type_expression),
    ),

    type_sum: $ => prec.dynamic(-1, seq(
      optional('|'),
      $.type_variant,
      repeat(seq('|', $.type_variant)),
    )),

    type_variant: $ => prec(1, seq(
      field('name', $.identifier),
      optional(seq(
        '(',
        $._type_expression,
        repeat(seq(',', $._type_expression)),
        optional(','),
        ')',
      )),
    )),

    parenthesized_type: $ => seq('(', $._type_expression, ')'),

    // -----------------------------------------------------------------------
    // Expressions (ordered by precedence)
    // -----------------------------------------------------------------------
    _expression: $ => choice(
      $.integer_literal,
      $.float_literal,
      $.string_literal,
      $.template_literal,
      $.boolean_literal,
      $.identifier,
      $.record_literal,
      $.array_literal,
      $.parenthesized_expression,
      $.lambda_expression,
      $.if_expression,
      $.match_expression,
      $.block_expression,
      $.field_access_expression,
      $.index_expression,
      $.call_expression,
      $.binary_expression,
      $.unary_expression,
    ),

    // -- Literal expressions --
    parenthesized_expression: $ => seq(
      '(',
      optional(seq(
        $._expression,
        repeat(seq(',', $._expression)),
        optional(','),
      )),
      ')',
    ),

    array_literal: $ => seq(
      '[',
      optional(seq(
        $._expression,
        repeat(seq(',', $._expression)),
        optional(','),
      )),
      ']',
    ),

    record_literal: $ => seq(
      '{',
      optional(seq(
        $.record_field,
        repeat(seq(',', $.record_field)),
        optional(','),
      )),
      '}',
    ),

    record_field: $ => seq(
      field('name', $.identifier),
      ':',
      field('value', $._expression),
    ),

    // -- Lambda --
    lambda_expression: $ => prec(1, seq(
      $._lambda_params,
      '=>',
      field('body', $._expression),
    )),

    _lambda_params: $ => choice(
      $.identifier,
      $.lambda_param_list,
    ),

    lambda_param_list: $ => seq(
      '(',
      optional(seq(
        $.lambda_param,
        repeat(seq(',', $.lambda_param)),
        optional(','),
      )),
      ')',
    ),

    lambda_param: $ => seq(
      field('name', $.identifier),
      optional(seq(':', field('type', $._type_expression))),
    ),

    // -- If expression --
    if_expression: $ => prec.left(2, seq(
      'if',
      field('condition', $._expression),
      field('consequence', $.block_expression),
      'else',
      field('alternative', choice($.block_expression, $.if_expression)),
    )),

    // -- Match expression --
    match_expression: $ => seq(
      'match',
      field('subject', $._expression),
      field('body', $.match_block),
    ),

    match_block: $ => seq(
      '{',
      repeat($.match_arm),
      '}',
    ),

    match_arm: $ => seq(
      field('pattern', $._pattern),
      '=>',
      field('value', $._expression),
    ),

    // -- Block expression --
    block_expression: $ => seq(
      '{',
      repeat($._statement),
      optional($._expression),
      '}',
    ),

    _statement: $ => seq(
      choice(
        prec(1, $.let_binding),
        prec(0, $._expression),
      ),
      optional(';'),
    ),

    // -- Field access: a.b --
    field_access_expression: $ => prec(10, seq(
      field('object', $._expression),
      '.',
      field('field', $.identifier),
    )),

    // -- Index: a[b] --
    index_expression: $ => prec(9, seq(
      field('array', $._expression),
      '[',
      field('index', $._expression),
      ']',
    )),

    // -- Call: f(a, b) --
    call_expression: $ => prec(8, seq(
      field('function', $._expression),
      '(',
      optional(seq(
        $._expression,
        repeat(seq(',', $._expression)),
        optional(','),
      )),
      ')',
    )),

    // -- Unary: !x, -x --
    unary_expression: $ => prec(7, seq(
      choice('!', '-'),
      field('operand', $._expression),
    )),

    // -- Binary: arithmetic, comparison, logical (precedence order) --
    binary_expression: $ => choice(
      // Multiplicative
      prec.left(6, seq(
        field('left', $._expression),
        field('operator', choice('*', '/', '%')),
        field('right', $._expression),
      )),
      // Additive
      prec.left(5, seq(
        field('left', $._expression),
        field('operator', choice('+', '-')),
        field('right', $._expression),
      )),
      // Comparison
      prec.left(4, seq(
        field('left', $._expression),
        field('operator', choice('==', '!=', '<', '<=', '>', '>=')),
        field('right', $._expression),
      )),
      // Logical AND
      prec.left(3, seq(
        field('left', $._expression),
        field('operator', '&&'),
        field('right', $._expression),
      )),
      // Logical OR
      prec.left(2, seq(
        field('left', $._expression),
        field('operator', '||'),
        field('right', $._expression),
      )),
    ),

    // -----------------------------------------------------------------------
    // Patterns (for match arms)
    // -----------------------------------------------------------------------
    _pattern: $ => choice(
      $.wildcard_pattern,
      $.binding_pattern,
      $.literal_pattern,
      $.constructor_pattern,
      $.tuple_pattern,
      $.record_pattern,
    ),

    wildcard_pattern: $ => '_',

    binding_pattern: $ => $.identifier,

    literal_pattern: $ => choice(
      $.integer_literal,
      $.float_literal,
      $.string_literal,
      $.template_literal,
      $.boolean_literal,
    ),

    constructor_pattern: $ => seq(
      field('name', $.identifier),
      '(',
      field('args', $._pattern),
      repeat(seq(',', field('args', $._pattern))),
      optional(','),
      ')',
    ),

    tuple_pattern: $ => seq(
      '(',
      $._pattern,
      repeat(seq(',', $._pattern)),
      optional(','),
      ')',
    ),

    record_pattern: $ => seq(
      '{',
      optional(seq(
        $.record_pattern_field,
        repeat(seq(',', $.record_pattern_field)),
        optional(','),
      )),
      '}',
    ),

    record_pattern_field: $ => seq(
      field('name', $.identifier),
      optional(seq(':', field('pattern', $._pattern))),
    ),
  },
})
