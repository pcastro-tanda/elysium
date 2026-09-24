# Style/FrozenStringLiteralComment

Add the frozen_string_literal comment to the top of files to help transition to frozen string literals by default.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | nursery |

Helps you transition from mutable string literals to frozen string
literals. It will add the `# frozen_string_literal: true` magic comment to
the top of files to enable frozen string literals. Frozen string literals
may be default in future Ruby. The comment will be added below a shebang
and encoding comment. The frozen string literal comment is only valid in
Ruby 2.3+.

Note that the cop will accept files where the comment exists but is set to
`false` instead of `true` -- unless `EnforcedStyle: always_true` is used.

To require a blank line after this comment, see
`Layout/EmptyLineAfterMagicComment`.

```ruby
# EnforcedStyle: always (default)

# bad
module Bar
  # ...
end

# good
# frozen_string_literal: true

module Bar
  # ...
end

# good
# frozen_string_literal: false

module Bar
  # ...
end
```

```ruby
# EnforcedStyle: never

# bad
# frozen_string_literal: true

module Baz
  # ...
end

# good
module Baz
  # ...
end
```

```ruby
# EnforcedStyle: always_true

# bad
# frozen_string_literal: false

module Baz
  # ...
end

# bad
module Baz
  # ...
end

# good
# frozen_string_literal: true

module Bar
  # ...
end
```

Autocorrection is unsafe: any string mutation will change from being
accepted to raising `FrozenError`, since all strings become frozen by
default, and will need to be manually refactored.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `always` | `always`, `always_true`, `never` | Whether a `frozen_string_literal` comment is required, required and must be `true`, or forbidden. |

## Blind spots

Comments are read straight from physical source lines (RuboCop's own
`processed_source.tokens` is unavailable to us), so a file's leading
section is bounded by the first top-level statement's line rather than by
an actual non-comment token; this matches every realistic file but not,
say, one starting with a `BEGIN {}` block. Vim-style magic comments
(`# vim: ...`) are recognised but -- matching RuboCop's own
`VimComment#frozen_string_literal` -- never treated as specifying
`frozen_string_literal`. RuboCop's Ruby-version-gated "frozen by default"
fallback (relevant only to hypothetical future Ruby versions) is not
modelled; we match RuboCop's own current behaviour of treating it as
`false`. A `__END__` data section is not excluded when locating the first
top-level statement, unlike `Layout/TrailingWhitespace`.
