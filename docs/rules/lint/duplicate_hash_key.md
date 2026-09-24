# Lint/DuplicateHashKey

Checks for duplicated keys in hash literals.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for duplicated keys in hash literals. This cop considers both
primitive types and constants for the hash keys.

This cop mirrors a warning in Ruby 2.2.

```ruby
# bad
hash = { food: 'apple', food: 'orange' }

# good
hash = { food: 'apple', other_food: 'orange' }
```

## Options

This rule has no options.

## Blind spots

Keys are compared by rubocop-ast's `recursive_basic_literal?`/`const_type?`
rules (mirroring `Parser::AST::Node#eql?`'s `[type, children]` structural
equality, not source text), covering `nil`/`true`/`false`, integers,
floats, strings and symbols (plain or interpolated with literal-only
parts), regexps (their `i`/`x`/`m`/`o` flags and, when interpolated,
literal-only parts), arrays and hashes of such literals, `and`/`or` and the
comparison-operator/`!`/`*`/`<=>` sends rubocop-ast treats as recursively
literal, endless/beginless ranges of such literals, parenthesized
single-statement wrappers, and bare or namespaced constant references.
Rational and complex literals, backtick/`%x` command strings, and any
interpolation with zero or more than one embedded statement are never
recognized as comparable keys (rubocop-ast's `recursive_basic_literal?`
would still descend into some of these; false negatives here, never false
positives).
