#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <tree_sitter/parser.h>

enum TokenType {
  NEWLINE,
  INDENT,
  DEDENT,
  CONTINUATION,
  BLOCK_COLON,
  STRING_CONTENT,
  ERROR_SENTINEL,
};

#define MAX_NESTING 128

typedef struct {
  uint16_t levels[MAX_NESTING];
  uint16_t depth;
  uint16_t line_indent;
  bool pending_indent;
  bool pending_dedent;
  bool previous_block_colon;
  bool continuation_line;
} Scanner;

void *tree_sitter_vact_external_scanner_create(void) {
  Scanner *s = (Scanner *)calloc(1, sizeof(Scanner));
  if (s) { s->depth = 1; s->levels[0] = 0; }
  return s;
}

void tree_sitter_vact_external_scanner_destroy(void *payload) { free(payload); }

unsigned tree_sitter_vact_external_scanner_serialize(void *payload, char *buffer) {
  Scanner *s = (Scanner *)payload;
  unsigned n = 0;
  memcpy(buffer + n, &s->depth, sizeof(s->depth)); n += sizeof(s->depth);
  memcpy(buffer + n, s->levels, s->depth * sizeof(s->levels[0])); n += s->depth * sizeof(s->levels[0]);
  memcpy(buffer + n, &s->line_indent, sizeof(s->line_indent)); n += sizeof(s->line_indent);
  buffer[n++] = s->pending_indent ? 1 : 0;
  buffer[n++] = s->pending_dedent ? 1 : 0;
  buffer[n++] = s->previous_block_colon ? 1 : 0;
  buffer[n++] = s->continuation_line ? 1 : 0;
  return n;
}

void tree_sitter_vact_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {
  Scanner *s = (Scanner *)payload;
  memset(s, 0, sizeof(*s)); s->depth = 1;
  if (!buffer || !length) return;
  unsigned n = 0;
  if (length < sizeof(s->depth)) return;
  memcpy(&s->depth, buffer + n, sizeof(s->depth)); n += sizeof(s->depth);
  if (s->depth == 0 || s->depth > MAX_NESTING || length < n + s->depth * sizeof(s->levels[0]) + 6) {
    memset(s, 0, sizeof(*s)); s->depth = 1; return;
  }
  memcpy(s->levels, buffer + n, s->depth * sizeof(s->levels[0])); n += s->depth * sizeof(s->levels[0]);
  memcpy(&s->line_indent, buffer + n, sizeof(s->line_indent)); n += sizeof(s->line_indent);
  s->pending_indent = buffer[n++] != 0;
  s->pending_dedent = buffer[n++] != 0;
  s->previous_block_colon = buffer[n++] != 0;
  s->continuation_line = buffer[n] != 0;
}

static bool newline(Scanner *s, TSLexer *lexer, const bool *valid) {
  if (!valid[NEWLINE] || (lexer->lookahead != '\n' && lexer->lookahead != '\r')) return false;
  if (lexer->lookahead == '\r') { lexer->advance(lexer, false); if (lexer->lookahead == '\n') lexer->advance(lexer, false); }
  else lexer->advance(lexer, false);
  uint16_t width = 0;
  while (lexer->lookahead == '\t' || lexer->lookahead == ' ') {
    width++;
    lexer->advance(lexer, false);
  }
  s->line_indent = width;
  s->pending_dedent = false;
  s->pending_indent = lexer->lookahead != 0 && lexer->lookahead != '\n' && lexer->lookahead != '\r' && lexer->lookahead != '#';
  if (!s->pending_indent) s->line_indent = s->depth ? s->levels[s->depth - 1] : 0;
  lexer->mark_end(lexer);
  if (s->pending_indent && lexer->lookahead == '>' && width > s->levels[s->depth - 1] && !s->previous_block_colon && valid[CONTINUATION]) {
    lexer->advance(lexer, false);
    if (lexer->lookahead != '=') {
      lexer->mark_end(lexer);
      s->pending_indent = false;
      s->continuation_line = true;
      s->previous_block_colon = false;
      lexer->result_symbol = CONTINUATION;
      return true;
    }
    s->pending_indent = false;
  }
  lexer->result_symbol = NEWLINE;
  return true;
}

static bool layout(Scanner *s, TSLexer *lexer, const bool *valid) {
  if (!s->pending_indent && !s->pending_dedent) return false;
  uint16_t top = s->levels[s->depth - 1];
  if (s->pending_indent && lexer->lookahead == '>') {
    if (s->line_indent > top && !s->previous_block_colon && valid[CONTINUATION]) {
      lexer->advance(lexer, false);
      if (lexer->lookahead == '=') return false;
      lexer->mark_end(lexer);
      s->pending_indent = false;
      s->continuation_line = true;
      s->previous_block_colon = false;
      lexer->result_symbol = CONTINUATION;
      return true;
    }
    return false;
  }
  if (s->pending_indent && s->line_indent > top && s->previous_block_colon && valid[INDENT] && s->depth < MAX_NESTING) {
    s->levels[s->depth++] = s->line_indent;
    s->pending_indent = false;
    s->previous_block_colon = false;
    lexer->mark_end(lexer);
    lexer->result_symbol = INDENT;
    return true;
  }
  if (s->line_indent < top && valid[DEDENT] && s->depth > 1) {
    s->depth--;
    top = s->levels[s->depth - 1];
    s->pending_dedent = s->line_indent < top;
    s->pending_indent = !s->pending_dedent;
    s->previous_block_colon = false;
    lexer->mark_end(lexer);
    lexer->result_symbol = DEDENT;
    return true;
  }
  s->pending_indent = false;
  s->pending_dedent = false;
  s->previous_block_colon = false;
  return false;
}

bool tree_sitter_vact_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid) {
  Scanner *s = (Scanner *)payload;
  bool every_symbol_valid = true;
  for (unsigned i = 0; i <= ERROR_SENTINEL; i++) every_symbol_valid = every_symbol_valid && valid[i];
  if (every_symbol_valid) return false;
  if ((s->pending_indent || s->pending_dedent) && layout(s, lexer, valid)) return true;

  if (lexer->lookahead == 0 && valid[DEDENT] && s->depth > 1) {
    s->depth--;
    lexer->mark_end(lexer);
    lexer->result_symbol = DEDENT;
    return true;
  }

  if (valid[STRING_CONTENT]) {
    bool consumed = false;
    while (lexer->lookahead && lexer->lookahead != '"' && lexer->lookahead != '\\' && lexer->lookahead != '{' && lexer->lookahead != '\n' && lexer->lookahead != '\r') {
      lexer->advance(lexer, false);
      consumed = true;
    }
    if (!consumed) return false;
    lexer->mark_end(lexer);
    lexer->result_symbol = STRING_CONTENT;
    return true;
  }

  if (valid[BLOCK_COLON] || valid[NEWLINE]) {
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t') lexer->advance(lexer, true);
  }

  if (valid[BLOCK_COLON] && lexer->lookahead == ':') {
    lexer->advance(lexer, false);
    lexer->mark_end(lexer);
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t') lexer->advance(lexer, false);
    if (lexer->lookahead == 0 || lexer->lookahead == '\n' || lexer->lookahead == '\r' || lexer->lookahead == '#') {
      s->previous_block_colon = true;
      lexer->result_symbol = BLOCK_COLON;
      return true;
    }
    return false;
  }

  if (valid[NEWLINE]) return newline(s, lexer, valid);
  return false;
}
