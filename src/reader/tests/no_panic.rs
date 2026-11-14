//! The reader never panics: every line-boundary prefix of every spec block,
//! and hostile inputs.

use super::{rd, spec_blocks, DESIGN_MUSIC, LANG_REFERENCE};

#[test]
fn every_line_prefix_of_every_block_reads() {
    let mut blocks = spec_blocks(LANG_REFERENCE);
    blocks.extend(spec_blocks(DESIGN_MUSIC));
    assert_eq!(blocks.len(), 11);
    for block in &blocks {
        let mut end = 0;
        while let Some(k) = block[end..].find('\n') {
            end += k + 1;
            let _ = rd(&block[..end]);
            let _ = rd(block[..end].trim_end_matches('\n'));
        }
        // Char-boundary prefixes of the whole block cut lines mid-token.
        for (k, _) in block.char_indices().step_by(7) {
            let _ = rd(&block[..k]);
        }
    }
}

#[test]
fn hostile_inputs_read() {
    let inputs = [
        "",
        "\n\n\r\n",
        "\r",
        ":",
        "::",
        "->",
        ">",
        "&",
        "-",
        "?",
        "}",
        "]",
        "{",
        "[",
        "\"",
        "\"\\",
        "\"{",
        "\"{\"{\"{",
        "#",
        "#@",
        "\t\t\t",
        " \t x",
        "\t> x",
        "_",
        "_99999999999",
        "1/",
        "-1/-2",
        "..",
        "...",
        "a.",
        "a..b..c",
        "x: y: z:",
        "\u{1F3B5} \u{e9}",
        "import",
        "import a/b as",
        "let",
        "fn",
        "if\nelif\nelse\nelse",
        "a -> -> b",
        "{->}",
        "[&]",
        "f -",
        "\0\u{7f}",
    ];
    for src in inputs {
        let _ = rd(src);
    }
}
