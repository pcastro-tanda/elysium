# Style/MutableConstant

Do not assign mutable objects to constants.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks whether some constant value isn't a mutable literal (e.g. array or
hash).

```ruby
# bad
CONST = [1, 2, 3]

# good
CONST = [1, 2, 3].freeze
```

Strict mode (`EnforcedStyle: strict`) freezes all constants, not just
literals:

```ruby
# bad (strict mode only)
CONST = Something.new

# good
CONST = Something.new.freeze
```

Strict mode is considered experimental: it does not have an exhaustive list
of methods that produce frozen objects, so it has a decent chance of false
positives. There is no harm in freezing an already frozen object, though.

`Regexp` and `Range` literals have been frozen since Ruby 3.0 and are never
flagged. A `# shareable_constant_value: literal` (or `experimental_everything`
/`experimental_copy`) magic comment also suppresses offenses for the
constant writes it covers, matching Ruby's own Ractor shareable-constant
semantics.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `literals` | `literals`, `strict` | `literals` freezes only literal values assigned to constants; `strict` freezes every constant assignment. |

## Blind spots

Only plain `CONST = value` and `CONST ||= value` (and their `A::B::CONST`
path forms) are checked, matching RuboCop's own `on_casgn`: `+=`, `-=`,
`&&=`, other compound assignments, and multiple assignment (`A, B = x, y`)
are never inspected by RuboCop either, since their `casgn` target node's
parent is not an `or_asgn` node.

`TargetRubyVersion` is not modeled: this rule always assumes Ruby >= 3.0
semantics (`Regexp`/`Range` literals frozen, `frozen_string_literal` honored
per Ruby 3.0 rules). RuboCop's own Ruby <= 2.7 branches are unreachable when
parsing with Prism (they are tagged `unsupported_on: :prism` in RuboCop's
own spec suite), so this matches real behavior for every file this engine
can parse.

`shareable_constant_value` is read from Prism's native
`ShareableConstantNode` wrapping rather than re-scanning magic comments by
hand; this defers entirely to Prism's own (spec-verified) scoping instead of
reimplementing it.
