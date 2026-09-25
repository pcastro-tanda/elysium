# Lint/ConstantResolution

Checks that constants are fully qualified with `::`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

This is not enabled by default because it would mark a lot of offenses
unnecessarily.

Generally, gems should fully qualify all constants to avoid conflicts with
the code that uses the gem. Enable this cop without using `Only`/`Ignore`.

Large projects will over time end up with one or two constant names that
are problematic because of a conflict with a library or just internally
using the same name a namespace and a class. To avoid too many unnecessary
offenses, enable this cop with `Only: [The, Constant, Names, Causing, Issues]`.

NOTE: `Style/RedundantConstantBase` cop is disabled if this cop is enabled to
prevent conflicting rules. Because it respects user configurations that want
to enable this cop which is disabled by default.

```ruby
# By default checks every constant

# bad
User

# bad
User::Login

# good
::User

# good
::User::Login
```

With `Only: ['Login']` (restrict this cop to only being concerned about
certain constants):

```ruby
# bad
Login

# good
::Login

# good
User::Login
```

With `Ignore: ['Login']` (restrict this cop from being concerned about
certain constants):

```ruby
# bad
User

# good
::User::Login

# good
Login
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Only | `[]` |  | Restrict this cop to only looking at certain names. |
| Ignore | `[]` |  | Restrict this cop from looking at certain names. |

## Blind spots

Per upstream's own pattern shape, a `class Foo < Bar` superclass reference is
exempted exactly like the class's own name -- both are direct children of
the same `ClassNode` -- which this port reproduces rather than special-casing
away.
