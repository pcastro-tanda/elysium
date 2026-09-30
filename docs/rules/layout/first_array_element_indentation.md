# Layout/FirstArrayElementIndentation

Checks the indentation of the first element in an array literal.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks the indentation of the first element in an array literal where the
opening bracket and the first element are on separate lines. The other
elements' indentations are handled by `Layout/ArrayAlignment` cop.

This cop will respect `Layout/ArrayAlignment` and will not work when
`EnforcedStyle: with_fixed_indentation` is specified for `Layout/ArrayAlignment`.

By default, array literals that are arguments in a method call with
parentheses, and where the opening square bracket of the array is on the
same line as the opening parenthesis of the method call, shall have their
first element indented one step (two spaces) more than the position inside
the opening parenthesis.

Other array literals shall have their first element indented one step more
than the start of the line where the opening square bracket is.

This default style is called `special_inside_parentheses`.

```ruby
# EnforcedStyle: special_inside_parentheses (default)

# bad
array = [
  :value
]
and_in_a_method_call([
  :no_difference
                     ])

# good
array = [
  :value
]
but_in_a_method_call([
                        :its_like_this
                      ])
```

```ruby
# EnforcedStyle: consistent

# bad
array = [
  :value
]
but_in_a_method_call([
                        :its_like_this
])

# good
array = [
  :value
]
and_in_a_method_call([
  :no_difference
])
```

```ruby
# EnforcedStyle: align_brackets

# bad
and_now_for_something = [
                          :completely_different
]

# good
and_now_for_something = [
                          :completely_different
                        ]
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `special_inside_parentheses` | `special_inside_parentheses`, `consistent`, `align_brackets` | Whether an array literal argument's first element is indented relative to the preceding left parenthesis when the bracket shares its line (`special_inside_parentheses`), always relative to the start of the array's own line (`consistent`), or relative to the opening bracket's own column (`align_brackets`). |
| IndentationWidth | `nil` |  | Number of spaces for the first element's indentation, overriding `Layout/IndentationWidth`'s `Width` (which itself defaults to 2). |

## Blind spots

RuboCop's `each_argument_node` finds array arguments with `on_node(:array, arg, :send)`. Prism has no separate `block`/`csend` nodes, so this port treats a plain call as opaque except for its attached `do`/`{}` block, and descends into `&.` calls, which is how parser's tree shapes `on_node` in those cases.

RuboCop's `MultilineElementIndentation#right_sibling` is the pair's true next AST sibling regardless of type; this port's sibling lookahead (`record_facts_one_level`/`eager_check_pairs`) matches that (it looks at the next raw hash/keyword-hash element, not the next *pair*, so a `**splat` between two pairs is not skipped over).

The `ambiguous_style_detected`/`correct_style_detected`/`detected_styles` bookkeeping RuboCop's `MultilineElementIndentation` mixin performs (used only for `--auto-gen-config` style inference) is not replicated, since it never itself produces an offense in a single lint run.
