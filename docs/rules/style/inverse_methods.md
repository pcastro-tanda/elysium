# Style/InverseMethods

Use the inverse method instead of `!.method` if an inverse method is defined.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for usages of not (`not` or `!`) called on a method
when an inverse of that method can be used instead.

Methods that can be inverted by a not (`not` or `!`) should be defined
in `InverseMethods`.

Methods that are inverted by inverting the return
of the block that is passed to the method should be defined in
`InverseBlocks`.

@safety
  This cop is unsafe because it cannot be guaranteed that the method
  and its inverse method are both defined on receiver, and also are
  actually inverse of each other.

```ruby
# bad
!foo.none?
!foo.any? { |f| f.even? }
!foo.blank?
!(foo == bar)
foo.select { |f| !f.even? }
foo.reject { |f| f != 7 }

# good
foo.none?
foo.blank?
foo.any? { |f| f.even? }
foo != bar
foo == bar
!!('foo' =~ /^\w+$/)
!(foo.class < Numeric) # Checking class hierarchy is allowed
# Blocks with guard clauses are ignored:
foo.select do |f|
  next if f.zero?
  f != 1
end
```

## Options

This rule has no options.

## Blind spots

`InverseMethods`/`InverseBlocks` are read as a plain string-to-string
mapping (no schema default is declared for them, since map-shaped config
values have no `ConfigDefault` representation here); a project overriding
one without the other still gets this cop's real default for the other,
matching upstream's per-key `config/default.yml` merge. Detecting a `!`/
`not` ancestor (the double-negation and guard-clause-block skips) reads
the ancestor's own source text rather than a live parent node, since
`Context::ancestors` carries only kind and span; this is exact for this
shape (see the module docs) rather than an approximation.
