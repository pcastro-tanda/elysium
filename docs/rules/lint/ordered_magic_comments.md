# Lint/OrderedMagicComments

Checks the proper ordering of magic comments and whether a magic comment is not placed before a shebang.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks the proper ordering of magic comments and whether
a magic comment is not placed before a shebang.

@safety
  This cop's autocorrection is unsafe because file encoding may change.

```ruby
# bad

# frozen_string_literal: true
# encoding: ascii
p [''.frozen?, ''.encoding] #=> [true, #<Encoding:UTF-8>]

# good

# encoding: ascii
# frozen_string_literal: true
p [''.frozen?, ''.encoding] #=> [true, #<Encoding:US-ASCII>]

# good

#!/usr/bin/env ruby
# encoding: ascii
# frozen_string_literal: true
p [''.frozen?, ''.encoding] #=> [true, #<Encoding:US-ASCII>]
```

## Options

This rule has no options.

## Blind spots

`RuboCop::MagicComment` is reimplemented against raw physical lines rather
than upstream's parser-gem-backed token stream, and only the
`encoding`/`frozen_string_literal` subset of its keywords is ported (the
other two, `Lint/OrderedMagicComments` never queries `rbs_inline`,
`shareable_constant_value`, or `typed`).
