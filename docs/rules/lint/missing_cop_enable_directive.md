# Lint/MissingCopEnableDirective

Checks that there is a `# rubocop:enable ...` after a `# rubocop:disable ...`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks that there is an `# rubocop:enable ...` statement
after a `# rubocop:disable ...` statement. This will prevent leaving
cop disables on wide ranges of code, that latter contributors to
a file wouldn't be aware of.

You can set `MaximumRangeSize` to define the maximum number of
consecutive lines a cop can be disabled for.

- `.inf` any size (default)
- `0` allows only single-line disables
- `1` means the maximum allowed is as follows:

```ruby
# rubocop:disable SomeCop
a = 1
# rubocop:enable SomeCop
```

```ruby
# MaximumRangeSize: .inf (default)

# good
# rubocop:disable Layout/SpaceAroundOperators
x= 0
# rubocop:enable Layout/SpaceAroundOperators
# y = 1
# EOF

# bad
# rubocop:disable Layout/SpaceAroundOperators
x= 0
# EOF
```

```ruby
# MaximumRangeSize: 2

# good
# rubocop:disable Layout/SpaceAroundOperators
x= 0
# With the previous, there are 2 lines on which cop is disabled.
# rubocop:enable Layout/SpaceAroundOperators

# bad
# rubocop:disable Layout/SpaceAroundOperators
x= 0
x += 1
# Including this, that's 3 lines on which the cop is disabled.
# rubocop:enable Layout/SpaceAroundOperators
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| MaximumRangeSize | inf |  | Maximum number of consecutive lines the cop can be disabled for. |

## Blind spots

Ranges are grouped by each directive's own literal reference (`all`, a bare department word, or a
slash-qualified cop path) rather than by real, registry-expanded cop name -- see the module docs
for the full argument that this matches upstream for every case this cop's own spec covers, and
only diverges for the rare case a department-wide disable is later narrowed by re-enabling one of
its own specific members while leaving the rest disabled (upstream would still flag the other,
still-disabled members; this port considers the department reference closed as soon as anything
names it, department or member). `# rubocop:disable all` left open forever names `all` itself in
its message rather than upstream's unpredictable specific real cop name (registry expansion order),
an edge case with no meaningful upstream fidelity target.
