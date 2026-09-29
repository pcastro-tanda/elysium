# Lint/RedundantSafeNavigation

Checks for redundant safe navigation calls.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Use cases where a constant, named in camel case for classes and modules, is `nil` are rare, and an
offense is not detected when the receiver is a constant. The detection also applies to `self`, and
to literal receivers, except for `nil`.

For all receivers, the `instance_of?`, `kind_of?`, `is_a?`, `eql?`, `respond_to?`, and `equal?`
methods are checked by default. These are customizable with the `AllowedMethods` option.

The `AllowedMethods` option specifies nil-safe methods, i.e. methods that are allowed to skip safe
navigation.

The `InferNonNilReceiver` option specifies whether to look into previous code paths to infer if the
receiver can't be `nil`. This check is unsafe because the receiver can be redefined between the
safe navigation call and the previous regular method call. It does the inference only in the
current scope (e.g. within the same method definition).

The `AdditionalNilMethods` option specifies additional custom methods that are defined on
`NilClass`, used by `InferNonNilReceiver`.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedMethods | `instance_of?`, `kind_of?`, `is_a?`, `eql?`, `respond_to?`, `equal?` |  | Nil-safe methods allowed to skip safe navigation. |
| InferNonNilReceiver | false |  | Infer non-nil receivers from previous code in the same scope. |
| AdditionalNilMethods | `present?`, `blank?`, `try`, `try!` |  | Custom methods considered defined on `NilClass` for `InferNonNilReceiver`. |

## Blind spots

None recorded.
