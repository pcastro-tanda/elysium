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

With `AllCops: ActiveSupportExtensionsEnabled: true`, a call to one of
`DelegatingMethods` (`delegate` by default) that shadows an existing
definition is also flagged:

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

`Forwardable`'s `def_delegator`, `def_instance_delegator`, `def_delegators`
and `def_instance_delegators` define methods too, and are recognized
regardless of `ActiveSupportExtensionsEnabled`:

```ruby
# bad
class MyClass
  extend Forwardable

  def_delegator :delegation_target, :delegated_method_name

  def delegated_method_name
  end
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| DelegatingMethods | `delegate` |  | `delegate`-shaped macros whose arguments name defined methods. |

## Blind spots

Scope resolution (`Node#parent_module_name`) is reconstructed from a stack
pushed while walking the tree rather than by climbing ancestors on demand,
but follows the same rules: `class`/`module`/`class << expr` nesting,
`class_eval` (implicit or constant-receiver), and an argument-less
`Class.new`/`Module.new` block bound to a constant assignment all resolve;
a numbered-parameter or `it` block is passed over (upstream's ancestor
searches only see `:block` nodes); any other block (a `describe` block,
`.each`, a `Class.new` bound to a local variable, ...) makes the enclosing
scope unresolvable.

An anonymous `Class.new`/`Module.new` block is then a scope of its own
(`Object`, or `::Object` when even its own enclosing scope is
unresolvable), kept apart from other anonymous classes by RuboCop's
`anon_block_scope_id`: the source text of the enclosing call's receiver
plus its method name when there is one, and otherwise an identity unique to
the block. Because `Context::ancestors` carries only kinds and spans, that
parent is recovered from the Prism chain (`ArgumentsNode` for an argument,
`StatementsNode` for a body, with two or more statements standing for
whitequark's `begin`) and from the spans of the enclosing `def`/call nodes.

Cross-file duplicate detection is not ported: everything RuboCop 1.91 does
with `AllCops/UseProjectIndex` and the `rubydex` project index
(`AllowedCrossFilePaths`, the self-alias trick and Active Support's
`silence_redefinition_of_method`/`redefine_method` markers marking a
redefinition in *another* file as intentional) has no effect here, where
each file is linted on its own. Within one file those markers change
nothing upstream either.

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
