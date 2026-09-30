# Layout/ClosingHeredocIndentation

Checks the indentation of here document closings.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
class Foo
  def bar
    <<~SQL
      'Hi'
  SQL
  end
end

# good
class Foo
  def bar
    <<~SQL
      'Hi'
    SQL
  end
end

# bad

# heredoc contents is before closing heredoc.
foo arg,
    <<~EOS
  Hi
    EOS

# good
foo arg,
    <<~EOS
  Hi
EOS

# good
foo arg,
    <<~EOS
      Hi
    EOS
```

## Options

This rule has no options.

## Blind spots

None recorded.
