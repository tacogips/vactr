// Editor-side, deliberately flat superset of the Rust reader.
module.exports = grammar({
  name: 'vact',

  externals: $ => [
    $._newline,
    $._indent,
    $._dedent,
    $._continuation,
    $.block_colon,
    $.string_content,
    $._error_sentinel,
  ],

  extras: $ => [$.comment, $.directive, /[\t ]+/],
  rules: {
    source_file: $ => repeat(choice($._newline, $._statement)),
    _statement: $ => $.statement,
    statement: $ => prec.right(seq(
      repeat1($._item), repeat($.continuation),
      optional(seq($.block_colon, $.block)), optional($._newline),
    )),
    continuation: $ => prec.right(seq($._continuation, repeat1($._item), optional($._newline))),
    block: $ => seq(repeat($._newline), $._indent, repeat(choice($._newline, $._statement)), $._dedent),

    _item: $ => choice(
      $.pair, $.group, $.list, $.string, $.identifier,
      $.qualified_identifier, $.keyword, $.number, $.path, $.url,
      $.operator, $.wildcard, $.console_register,
    ),
    pair: $ => prec.right(seq(field('key', choice($.identifier, $.qualified_identifier, $.keyword)), ':', field('value', repeat1($._item)))),
    group: $ => seq('{', repeat($._item), '}'),
    list: $ => seq('[', repeat($._item), ']'),

    string: $ => seq('"', repeat(choice($.string_content, $.escape_sequence, $.interpolation)), '"'),
    escape_sequence: _ => token(/\\(?:["\\nt{}])/),
    interpolation: $ => seq('{', repeat($._item), '}'),

    comment: _ => token(prec(-1, /#[^\n]*/)),
    directive: _ => token(prec(1, /#@[^\n]*/)),
    identifier: _ => token(/[A-Za-z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)*/),
    qualified_identifier: _ => token(/[A-Za-z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)*\.[A-Za-z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)*/),
    keyword: _ => token(/:[A-Za-z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)*/),
    number: _ => token(/-?(?:[0-9]+\/[0-9]+|[0-9]+(?:\.[0-9]+)?)/),
    path: _ => token(/(?:\.\.\/|\.\/|~\/|\/[A-Za-z0-9._~-])[A-Za-z0-9._~/-]*/),
    url: _ => token(/[a-z][a-z0-9+.-]*:\/\/[^\s"#{}\[\]\\<>|^`]+/),
    wildcard: _ => token('_'),
    console_register: _ => token(/_[1-9][0-9]*/),
    operator: _ => token(/(?:->|>=|<=|\.\.|[+*/=<>&|?-])/),
  },
});
