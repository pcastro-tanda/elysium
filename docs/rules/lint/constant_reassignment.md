# Lint/ConstantReassignment

Checks for constant reassignments.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for constant reassignments.

Emulates Ruby's runtime warning "already initialized constant X" when a constant is reassigned in the same file and namespace.

The cop tracks constants defined via `NAME = value` syntax as well as class/module keyword definitions. It detects reassignment when a constant is first defined one way and then redefined using the `NAME = value` syntax.

The cop cannot catch all offenses, like, for example, when using metaprogramming (`Module#const_set`).

The cop cannot detect reassignment across files.

The cop only takes into account constants assigned in a "simple" way: directly inside class/module definition, or within another constant. Other type of assignments (e.g., inside a conditional) are disregarded.

The cop also tracks constant removal using `Module#remove_const` with symbol or string argument.

```ruby
# bad
X = :foo
X = :bar

# bad
class A
  X = :foo
  X = :bar
end

# bad
module A
  X = :foo
  X = :bar
end

# bad
class FooError < StandardError; end
FooError = Class.new(RuntimeError)

# bad
module M; end
M = 1

# good - keep only one assignment
X = :bar

class A
  X = :bar
end

module A
  X = :bar
end

# good - use OR assignment
X = :foo
X ||= :bar

# good - use conditional assignment
X = :foo
X = :bar unless defined?(X)

# good - remove the assigned constant first
class A
  X = :foo
  remove_const :X
  X = :bar
end
```

## Options

This rule has no options.

## Blind spots

No project-wide index is available, so reassignment of a constant previously defined in another file is never detected (upstream's `AllCops/UseProjectIndex` cross-file branch).
