# Layout/MultilineBlockLayout

Ensures newlines after multiline block do statements.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks whether the multiline do end blocks have a newline after the start of
the block. Additionally, it checks whether the block arguments, if any, are
on the same line as the start of the block. Putting block arguments on
separate lines, because the whole line would otherwise be too long, is
accepted.

```ruby
# bad
blah do |i| foo(i)
  bar(i)
end

# bad
blah do
  |i| foo(i)
  bar(i)
end

# good
blah do |i|
  foo(i)
  bar(i)
end

# bad
blah { |i| foo(i)
  bar(i)
}

# good
blah { |i|
  foo(i)
  bar(i)
}
```

## Options

This rule has no options.

## Blind spots

`block_arg_string`'s recursive-parenthesization and trailing-comma-preserving
autocorrection is ported exactly for the shapes RuboCop's own spec covers:
plain, splat, and destructured (`(a, b)`, nested arbitrarily deep) required
parameters, plus a single required parameter followed by a bare trailing
comma. A trailing comma nested inside a destructured group rather than at
the top level (`|(a,)|`) is not specially preserved, matching no known
upstream spec coverage either.
