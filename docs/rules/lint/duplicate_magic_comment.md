# Lint/DuplicateMagicComment

Checks for duplicated magic comments.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for duplicated magic comments.

```ruby
# bad

# encoding: ascii
# encoding: ascii

# good

# encoding: ascii

# bad

# frozen_string_literal: true
# frozen_string_literal: true

# good

# frozen_string_literal: true
```

## Options

This rule has no options.

## Blind spots

`RuboCop::MagicComment` is reimplemented against raw physical lines rather than upstream's parser-gem-backed token stream, and only the `encoding`/`frozen_string_literal` subset of its keywords is ported (the other two, `rbs_inline`/`shareable_constant_value`/`typed`, this cop never queries).
