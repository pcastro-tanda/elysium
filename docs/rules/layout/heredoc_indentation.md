# Layout/HeredocIndentation

Checks the indentation of the here document bodies.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

Checks the indentation of the here document bodies. The bodies are indented
one step.

NOTE: When `Layout/LineLength`'s `AllowHeredoc` is `false` (not default),
this cop does not add any offenses for long here documents to avoid
`Layout/LineLength`'s offenses.

```ruby
# bad
<<-RUBY
something
RUBY

# good
<<~RUBY
  something
RUBY
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| IndentationWidth | `nil` |  | Overrides `Layout/IndentationWidth`'s configured width for this cop alone; used both to check the body's indentation and, during autocorrection, to determine how many spaces should replace each tab. |

## Blind spots

RuboCop's `minimum_target_ruby_version 2.3` guard is not ported: the scenario it exists for (`RSpec` `:ruby22`) is itself marked `unsupported_on: :prism` upstream, since Prism -- the only parser this cop is ported against -- never targets a Ruby that old.
