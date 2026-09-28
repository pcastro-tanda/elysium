# Layout/IndentationStyle

Checks that the indentation method is consistent.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Either tabs only or spaces only are used for indentation.

```ruby
# EnforcedStyle: spaces (default)
# bad
# This example uses a tab to indent bar.
def foo
	bar
end

# good
# This example uses spaces to indent bar.
def foo
  bar
end
```

```ruby
# EnforcedStyle: tabs
# bad
# This example uses spaces to indent bar.
def foo
  bar
end

# good
# This example uses a tab to indent bar.
def foo
	bar
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `spaces` | `spaces`, `tabs` | Which whitespace character indentation must consist of. |
| IndentationWidth | `nil` |  | Number of spaces a tab is replaced by (or that make up a tab) during autocorrection. Defaults to `Layout/IndentationWidth`'s `Width`. |

## Blind spots

The line scan runs over every line up to (but not including) a trailing
`__END__` data section, and is not otherwise aware of node boundaries: a
comment line, or the first line of a multi-line string literal (before its
opening delimiter), is compared exactly as upstream does, since neither is
a `:str`/`:dstr` range.
