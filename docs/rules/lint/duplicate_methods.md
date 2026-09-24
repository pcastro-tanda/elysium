# Lint/DuplicateMethods

Checks for duplicated instance (or singleton) method definitions.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for duplicated instance (or singleton) method definitions.

Aliasing a method to itself is allowed, as it indicates that the developer
intends to suppress Ruby's method redefinition warnings.

```ruby
# bad
def foo
  1
end

def foo
  2
end

# bad
def foo
  1
end

alias foo bar

# good
def foo
  1
end

def bar
  2
end

# good
def foo
  1
end

alias bar foo

# good
alias foo foo
def foo
  1
end

# good
alias_method :foo, :foo
def foo
  1
end
```

With `AllCops: ActiveSupportExtensionsEnabled: true`, a `delegate` call that
shadows an existing definition is also flagged:

```ruby
# bad
def foo
  1
end

delegate :foo, to: :bar

# good
def foo
  1
end

delegate :baz, to: :bar
```

## Options

This rule has no options.

## Blind spots

Scope resolution (`Node#parent_module_name`) is reconstructed from a stack
pushed while walking the tree rather than by climbing ancestors on demand,
but follows the same rules: `class`/`module`/`class << expr` nesting,
`class_eval` (implicit or constant-receiver), and a `Class.new`/`Module.new`
block bound to a constant assignment all resolve; any other block (a
`describe` block, `.each`, a `Class.new` bound to a local variable, ...)
makes the enclosing scope unresolvable, matching RuboCop's own behavior of
silently skipping definitions whose scope it cannot determine.

The `rescue`/`ensure` one-free-redefinition allowance
(`found_method`'s `@scopes`) is keyed only by clause kind, exactly as
upstream: two unrelated methods redefined once each across all of a file's
`rescue` clauses share the same allowance bucket.

`def A.foo`/`Foo::Bar.foo`-style constant receivers are resolved by matching
the constant's simple name against enclosing `class`/`module`/dynamic
`casgn` scopes (RuboCop's `lookup_constant`); a receiver naming an unrelated
top-level constant is not resolved to it (RuboCop does not attempt
whole-program constant resolution either).

`source_location` uses the linted file's own path as given (RuboCop's
`smart_path`, relative to `Dir.pwd`, is not replicated: a diagnostic
constructed with an absolute or repo-relative path that differs from the
path the file was read under will not match).
