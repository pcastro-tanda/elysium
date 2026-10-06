# Rails/PluckInWhere

Use `select` instead of `pluck` in `where` query methods.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies places where `pluck` is used in `where` query methods and suggests using `select` instead.

Since `pluck` is an eager method and hits the database immediately, using `select` helps to avoid additional database queries.

This cop has two different enforcement modes. When the EnforcedStyle is conservative (the default) then only calls to `pluck` on a constant (i.e. a model class) in the `where` is used as offenses.

When the EnforcedStyle is aggressive then all calls to `pluck` in the `where` are considered offenses.

This cop's autocorrection is unsafe because it may change the result when the receiver is not an Active Record relation.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `conservative` | `conservative`, `aggressive` | `conservative` only reports `pluck` called on a constant. |

## Blind spots

None recorded.
