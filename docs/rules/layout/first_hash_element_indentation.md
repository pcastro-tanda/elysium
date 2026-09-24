# Layout/FirstHashElementIndentation

Checks the indentation of the first key in a hash literal.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks the indentation of the first key in a hash literal where the opening
brace and the first key are on separate lines. The other keys' indentations
are handled by `Layout/HashAlignment`.

By default, `Hash` literals that are arguments in a method call with
parentheses, and where the opening curly brace of the hash is on the same
line as the opening parenthesis of the method call, shall have their first
key indented one step (two spaces) more than the position inside the opening
parenthesis. Other hash literals shall have their first key indented one
step more than the start of the line where the opening curly brace is. This
default style is called `special_inside_parentheses`.

```ruby
# EnforcedStyle: special_inside_parentheses (default)

# bad
hash = {
  key: :value
}
and_in_a_method_call({
  no: :difference
                     })

# good
special_inside_parentheses
hash = {
  key: :value
}
but_in_a_method_call({
                        its_like: :this
                      })
```

```ruby
# EnforcedStyle: consistent

# bad
hash = {
  key: :value
}
but_in_a_method_call({
                        its_like: :this
                       })

# good
hash = {
  key: :value
}
and_in_a_method_call({
  no: :difference
})
```

```ruby
# EnforcedStyle: align_braces

# bad
and_now_for_something = {
                          completely: :different
}

# good
and_now_for_something = {
                          completely: :different
                        }
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `special_inside_parentheses` | `special_inside_parentheses`, `consistent`, `align_braces` | Whether a hash literal argument's first key is indented relative to the preceding left parenthesis when the brace shares its line (`special_inside_parentheses`), always relative to the start of the hash's own line (`consistent`), or relative to the opening brace's own column (`align_braces`). |
| IndentationWidth | `nil` |  | Number of spaces for the first key's indentation, overriding `Layout/IndentationWidth`'s `Width` (which itself defaults to 2). |

## Blind spots

RuboCop's `each_argument_node` resolves the governing call for a hash argument via a fully generic recursive `on_node` search that descends through *any* wrapper node type (arrays, ternaries, boolean connectives, splats, method chains, ...) stopping only at a nested `send`/`csend`. This port narrows that search to hash-literal/keyword-hash-literal pair chains only (`eager_check_call_hash`/`eager_check_pairs`): a hash argument buried inside e.g. an array literal or a ternary is treated as an ordinary top-level hash literal (checked against the start of its own line) rather than against the call's parenthesis, a false-negative-only divergence for the `special_inside_parentheses`/`consistent` distinction in that shape.

RuboCop's `MultilineElementIndentation#right_sibling` is the pair's true next AST sibling regardless of type; this port's sibling lookahead (`record_facts_one_level`/`eager_check_pairs`) matches that (it looks at the next raw hash/keyword-hash element, not the next *pair*, so a `**splat` between two pairs is not skipped over).

The `ambiguous_style_detected`/`correct_style_detected`/`detected_styles` bookkeeping RuboCop's `MultilineElementIndentation` mixin performs (used only for `--auto-gen-config` style inference) is not replicated, since it never itself produces an offense in a single lint run.
