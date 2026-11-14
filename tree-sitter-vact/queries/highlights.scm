(comment) @comment
(directive) @comment.directive
(string) @string
(string_content) @string
(escape_sequence) @string
(number) @number
(keyword) @string.special.symbol
(path) @string.special.path
(url) @string.special.path
((statement . (identifier) @keyword) (#any-of? @keyword "let" "var" "upd" "fn" "inst" "bus" "look" "master" "import" "slot" "if"))
((group . (identifier) @keyword) (#any-of? @keyword "let" "var" "upd" "fn" "inst" "bus" "look" "master" "import" "slot" "if"))
["{" "}" "[" "]"] @punctuation.bracket
