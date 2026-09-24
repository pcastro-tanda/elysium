# Layout/IndentationWidth

Checks for indentation that doesn't use the specified number of spaces.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

The indentation width can be configured using the `Width` setting. The default width is 2.

See also the `Layout/IndentationConsistency` cop which is the companion to this one.

```ruby
# bad
class A
 def test
  puts 'hello'
 end
end

# good
class A
  def test
    puts 'hello'
  end
end
```

Lines that match `AllowedPatterns` are not required to follow the configured width:

```ruby
# AllowedPatterns: ['^\s*module']

# bad
module A
class B
  def test
  puts 'hello'
  end
end
end

# good
module A
class B
  def test
    puts 'hello'
  end
end
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Width | 2 |  | Number of spaces for each indentation level. |
| AllowedPatterns | `[]` |  | Lines matching one of these patterns are not required to follow the configured width. |

## Blind spots

Does not track parent pointers, so `leftmost_modifier_of` (chained bare-modifier calls such as
`foo private def bar; end`) approximates with the innermost call's own location; only a single
level of `modifier def` nesting is verified against fixtures. `Layout/AccessModifierIndentation`'s
`macro?`/`in_macro_scope?` nuance is approximated by a simple bare-call-name check (no scope
verification). Autocorrection does not special-case parenthesized multi-statement bodies (RuboCop's
`parentheses?` guard); it always narrows to the first statement. Non-heredoc multi-line string/
symbol literals are not added to the autocorrect taboo ranges (only heredoc bodies are), so a
reindented statement that embeds a multi-line plain string could shift that string's continuation
lines; this has not triggered in the fixture set.
