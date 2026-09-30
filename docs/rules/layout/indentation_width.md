# Layout/IndentationWidth

Checks for indentation that doesn't use the specified number of spaces.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

The indentation width can be configured using the `Width` setting. The default width is 2.
The block body indentation for method chain blocks can be configured using the
`EnforcedStyleAlignWith` setting.

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
```

A multi-line parenthesized grouping expression has its body indented one step from the line
the opening parenthesis is on:

```ruby
# bad
value = (
foo - bar
)

# good
value = (
  foo - bar
)
```

```ruby
# EnforcedStyleAlignWith: start_of_line (default)
records.uniq { |el| el[:profile_id] }
       .map do |message|
  SomeJob.perform_later(message[:id])
end

# EnforcedStyleAlignWith: relative_to_receiver
records.uniq { |el| el[:profile_id] }
       .map do |message|
         SomeJob.perform_later(message[:id])
       end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Width | 2 |  | Number of spaces for each indentation level. |
| EnforcedStyleAlignWith | `start_of_line` | `start_of_line`, `relative_to_receiver` | Whether a method-chain block's body is indented relative to the start of the line the block starts on (`start_of_line`) or relative to the method call's position in the chain (`relative_to_receiver`). |
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
lines; this has not triggered in the fixture set. A `rescue`/`ensure` body's autocorrect target
is only the leading statements, where upstream's whitequark `:rescue`/`:ensure` node spans the
clause keywords and their bodies too, so correcting a misindented `begin`/`rescue`/`ensure`
leading body does not drag the clause keywords along with it. `other_offense_in_same_range?`
state is also cleared at the start of every autocorrect round, where upstream's is never reset,
so a converged correction can differ from upstream's (upstream suppresses later rounds'
corrections with stale byte ranges).
