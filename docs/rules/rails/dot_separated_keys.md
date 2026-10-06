# Rails/DotSeparatedKeys

Enforces the use of dot-separated keys instead of `:scope` options in `I18n` translation methods.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the use of dot-separated locale keys instead of specifying the `:scope` option with an array or a single symbol in `I18n` translation methods. Dot-separated notation is easier to read and trace the hierarchy.

```ruby
# bad
I18n.t :record_invalid, scope: [:activerecord, :errors, :messages]
I18n.t :title, scope: :invitation

# good
I18n.t 'activerecord.errors.messages.record_invalid'
I18n.t :record_invalid, scope: 'activerecord.errors.messages'
```

## Options

This rule has no options.

## Blind spots

Scope elements that are `true`, `false`, `nil`, rationals, complex numbers or non-decimal integers are treated as non-literal (no offense).
