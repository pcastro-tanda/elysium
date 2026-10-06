# Rails/DuplicateScope

Multiple scopes share this same expression.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for multiple scopes in a model that have the same expression. This often means you copy/pasted a scope, updated the name, and forgot to change the condition.

```ruby
# bad
scope :visible, -> { where(visible: true) }
scope :hidden, -> { where(visible: true) }

# good
scope :visible, -> { where(visible: true) }
scope :hidden, -> { where(visible: false) }
```

## Options

This rule has no options.

## Blind spots

Scope expressions are compared by a structural fingerprint (node kinds, with each node's own punctuation and names, ignoring whitespace) rather than by RuboCop's AST equality, so e.g. `'a'` and `"a"` differ. A class body with `rescue` is not inspected.
