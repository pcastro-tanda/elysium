# Lint/UselessElseWithoutRescue

Checks for useless `else` in `begin..end` without `rescue`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

`begin`/`end` blocks that have an `else` but no `rescue` are useless: the `else` branch runs unconditionally, exactly like the code preceding it would, since it only ever executes when no exception was raised. This is not valid syntax on Ruby 2.6 or higher.

```ruby
# bad
begin
do_something
else
do_something_else # This will never be run.
end

# good
begin
do_something
rescue
handle_errors
else
do_something_else
end
```

## Options

This rule has no options.

## Blind spots

Upstream fires from its own parser's `:useless_else` diagnostic, which covers `begin/else/end` without `rescue` written anywhere a `begin` can appear (including implicit method/block bodies). On Ruby 2.6+ this construct is a syntax error under Prism too, and this engine reports only `Lint/Syntax` and skips the node walk entirely for any file with a parse error, so this rule can never actually fire in practice.
