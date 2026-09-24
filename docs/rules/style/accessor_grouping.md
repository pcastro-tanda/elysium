# Style/AccessorGrouping

Checks for grouping of accessors in `class` and `module` bodies.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

By default it enforces accessors to be placed in grouped declarations, but it
can be configured to enforce separating them in multiple declarations.

If there is a method call before the accessor method it is always allowed,
as it might be intended, e.g. for a Sorbet `sig` block. If there is an
RBS::Inline annotation comment (`#: String`) just after the accessor method
it is always allowed too.

```ruby
# bad
class Foo
  attr_reader :bar
  attr_reader :baz
end

# good
class Foo
  attr_reader :bar, :baz
end
```

```ruby
# EnforcedStyle: separated
# bad
class Foo
  attr_reader :bar, :baz
end

# good
class Foo
  attr_reader :bar
  attr_reader :baz
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `grouped` | `grouped`, `separated` | Whether accessors of the same kind should be grouped into one declaration, or each placed in its own. |

## Blind spots

Comment attachment for `separated`'s per-argument comment carry-over is a
direct port of `Parser::Source::Comment::Associator`'s leading/decorating
rules specialised to a flat list of leaf arguments (symbols/splats): a
comment inside a compound argument expression (unlikely for an attr-list
element) is not independently re-attached to that argument's own
sub-expressions. `same_line?` between an RBS::Inline comment and the
preceding statement compares first lines only, matching every case this cop
can actually see (the preceding statement is always a single accessor call
or access-modifier/macro call).
