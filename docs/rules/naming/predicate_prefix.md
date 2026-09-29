# Naming/PredicatePrefix

Checks that predicate method names end with a question mark and do not start with a forbidden prefix.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

A method is determined to be a predicate method if its name starts with one of the prefixes listed in the `NamePrefix` configuration. The list defaults to `is_`, `has_`, `have_`, and `does_` but may be overridden.

Predicate methods must end with a question mark.

When `ForbiddenPrefixes` is also set (as it is by default), predicate methods which begin with a forbidden prefix are not allowed, even if they end with a `?`. These methods should be changed to remove the prefix.

When `UseSorbetSigs` is set to `true` (optional), the cop only reports offenses if the method has a Sorbet `sig` with a return type of `T::Boolean`.

```ruby
# bad
def is_even(value)
end

# good (ForbiddenPrefixes: ['is_'])
def even?(value)
end

# good (ForbiddenPrefixes: [])
def is_even?(value)
end
```

With `AllowedMethods: ['is_a?']` (the default):

```ruby
# good, despite starting with the `is_` prefix
def is_a?(value)
end
```

With `MethodDefinitionMacros: ['define_method', 'define_singleton_method']` (the default):

```ruby
# bad
define_method(:is_even) { |value| }

# good
define_method(:even?) { |value| }
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| NamePrefix | `is_`, `has_`, `have_`, `does_` |  | Predicate name prefixes. |
| ForbiddenPrefixes | `is_`, `has_`, `have_`, `does_` |  | Predicate name prefixes that should be removed. |
| AllowedMethods | `is_a?` |  | Predicate names which, despite having a forbidden prefix, or no `?`, should still be accepted. |
| MethodDefinitionMacros | `define_method`, `define_singleton_method` |  | Method definition macros for dynamically generated methods. |
| UseSorbetSigs | false |  | Use Sorbet's `T::Boolean` return type to detect predicate methods. |

## Blind spots

`AllCops/UseProjectIndex` cross-file override detection is not ported: it needs the `rubydex` gem's project index, which elysium has no equivalent of. Methods that override an ancestor method defined elsewhere in the project are always reported here, even when upstream would suppress them.
