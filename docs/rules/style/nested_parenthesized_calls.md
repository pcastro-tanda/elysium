# Style/NestedParenthesizedCalls

Parenthesize method calls which are nested inside the argument list of another parenthesized method call.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for unparenthesized method calls in the argument list of a parenthesized method call.
`be`, `be_a`, `be_an`, `be_between`, `be_falsey`, `be_kind_of`, `be_instance_of`, `be_truthy`,
`be_within`, `eq`, `eql`, `end_with`, `include`, `match`, `raise_error`, `respond_to`, and
`start_with` methods are allowed by default. These are customizable with the `AllowedMethods`
option.

```ruby
# good
method1(method2(arg))

# bad
method1(method2 arg)
```

With `AllowedMethods: [foo]`:

```ruby
# good
method1(foo arg)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedMethods | `be`, `be_a`, `be_an`, `be_between`, `be_falsey`, `be_kind_of`, `be_instance_of`, `be_truthy`, `be_within`, `eq`, `eql`, `end_with`, `include`, `match`, `raise_error`, `respond_to`, `start_with` |  | Method names always allowed to be called unparenthesized as the sole argument of a call that itself is the parenthesized call's sole argument. |

## Blind spots

None recorded.
