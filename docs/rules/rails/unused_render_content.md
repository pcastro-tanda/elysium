# Rails/UnusedRenderContent

Do not specify body content for a response with a non-content status code.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

If you try to render content along with a non-content status code (100-199, 204, 205, or 304), it will be dropped from the response.

This cop checks for uses of `render` which specify both body content and a non-content status.

```ruby
# bad
render 'foo', status: :continue
render status: 100, plain: 'Ruby!'

# good
head :continue
head 100
```

## Options

This rule has no options.

## Blind spots

The non-content status symbols are those of Rack 2.2 and later.
