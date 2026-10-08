# Layout/LineEndStringConcatenationIndentation

Checks the indentation of the next line after a line that ends with a string literal and a backslash.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

If `EnforcedStyle: aligned` is set, the concatenated string parts shall be aligned with the
first part. There are some exceptions, such as implicit return values, where the concatenated
string parts shall be indented regardless of `EnforcedStyle` configuration.

If `EnforcedStyle: indented` is set, it's the second line that shall be indented one step more
than the first line. Lines 3 and forward shall be aligned with line 2.

```ruby
# bad
def some_method
  'x' \
  'y' \
  'z'
end

my_hash = {
  first: 'a message' \
    'in two parts'
}

# good
def some_method
  'x' \
    'y' \
    'z'
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `aligned` | `aligned`, `indented` | The indentation style to enforce for a backslash-continued string concatenation. |
| IndentationWidth | `nil` |  | Overrides `Layout/IndentationWidth`'s configured width for this cop alone, for `EnforcedStyle: indented`. |

## Blind spots

None recorded.
