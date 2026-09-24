# Lint/EmptyBlock

Checks for blocks without a body.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Such empty blocks are typically an oversight or we should provide a comment
to clarify what we're aiming for.

Empty lambdas and procs are ignored by default.

NOTE: For backwards compatibility, the configuration that allows/disallows
empty lambdas and procs is called `AllowEmptyLambdas`, even though it also
applies to procs.

```ruby
# bad
items.each { |item| }

# good
items.each { |item| puts item }
```

With `AllowComments: true` (default), a block/lambda with a comment inside
it or trailing it on the same line is left alone:

```ruby
# good
items.each do |item|
  # TODO: implement later (inner comment)
end

items.each { |item| } # TODO: implement later (inline comment)
```

With `AllowEmptyLambdas: true` (default), `-> { }`, `lambda do end`,
`proc { }`, and `Proc.new { }` are all left alone.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowComments | true |  | Allow a block/lambda with a comment inside or trailing it. |
| AllowEmptyLambdas | true |  | Allow an empty `->`/`lambda`/`proc`/`Proc.new`. |

## Blind spots

`comment_disables_cop?` (a `# rubocop:disable`/`todo` comment for this cop
specifically, on the node's own first line, does not itself count as an
`AllowComments` explanation) is not special-cased: the engine's own
directive handling drops that diagnostic anyway once reported, so the final
diagnostics are identical either way.
