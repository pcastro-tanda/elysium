# Layout/InitialIndentation

Checks for indentation of the first non-blank non-comment line in a file.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for indentation of the first non-blank non-comment line in a file.

```ruby
# bad
   class A
     def foo; end
   end

# good
class A
  def foo; end
end
```

## Options

This rule has no options.

## Blind spots

Upstream measures the offense's length as the parser's first lexical token
(`processed_source.tokens.find { |t| !t.text.start_with?('#') }`). Without a
token stream, this port re-lexes just that one token from the source bytes
at the AST's first-statement offset (which Prism already anchors past any
leading whitespace, comments, and byte order mark). The mini-lexer covers
identifiers/keywords, numbers, strings, symbols, `@`/`@@`/`$` variables, and
the common operators; an exotic first token (e.g. a `%`-literal with unusual
delimiters) may get a slightly off highlight width, though the reported
offset, message, and autocorrect are unaffected.
