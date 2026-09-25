# Lint/UselessAccessModifier

Checks for redundant access modifiers.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for redundant access modifiers, including those with no code, those
which are repeated, those which are on top-level, and leading `public`
modifiers in a class or module body. Conditionally-defined methods are
considered as always being defined, and thus access modifiers guarding such
methods are not redundant.

This cop has a `ContextCreatingMethods` option, an array of methods which,
when called, are known to create their own context in the module's current
access context (e.g. ActiveSupport's `concerning`). It also has a
`MethodCreatingMethods` option, an array of methods which, when called, are
known to create other methods in the module's current access context (e.g.
ActiveSupport's `delegate`). Both default to an empty array.

```ruby
# bad
class Foo
  public # this is redundant (default access is public)

  def method
  end
end

# bad
class Foo
  # The following is redundant (methods defined on the class' singleton
  # class are not affected by the private modifier)
  private

  def self.method3
  end
end

# bad
class Foo
  private # this is redundant (no following methods are defined)
end

# bad
private # this is useless (access modifiers have no effect on top-level)

def method
end

# good
class Foo
  private # this is not redundant (a method is defined)

  def method2
  end
end

# good
class Foo
  # The following is not redundant (conditionally defined methods are
  # considered as always defining a method)
  private

  if condition?
    def method
    end
  end
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| ContextCreatingMethods | `[]` |  | Methods which, when called with a block, are known to create their own context in the module's current access context. |
| MethodCreatingMethods | `[]` |  | Methods which, when called, are known to create other methods in the module's current access context. |

## Blind spots

Ported as a single-pass stack of scope frames rather than RuboCop's own
double-recursion (its manual `check_child_nodes` walk plus the independent
`on_class`/`on_block` visitor callbacks, reconciled only by
`Base#add_offense` deduplicating identical ranges); the observable set of
offenses is the same, but nothing here depends on RuboCop's own
double-reporting quirk. `ContextCreatingMethods`/`MethodCreatingMethods`
entries are matched as plain method names (RuboCop's `def_node_matcher`-
generated matchers accept the same shapes this port checks directly:
`nil?`/const receiver plus an attached block for context-creating methods,
a `nil?` receiver for method-creating ones); a configured `"included"`
entry is ignored either way, matching RuboCop's own guard against
redefining its `included_block?`/`method_definition?` matchers.
`AllCops: ActiveSupportExtensionsEnabled` gates `included do ... end`
exactly as RuboCop does, including the resulting difference in which of two
repeated modifiers gets flagged. `private_class_method` called with
arguments reproduces RuboCop's own `cur_vis, unused = nil` quirk (a
non-array implicit return destructured by Ruby's parallel assignment):
after such a call, the enclosing scope can never again report a repeated
bare modifier, since `cur_vis` stays `nil` for the rest of that scope.
