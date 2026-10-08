# Rails/RenderPlainText

Prefer `render plain:` over `render text:`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where `render text:` can be replaced with `render plain:`.

```ruby
# bad - explicit MIME type to `text/plain`
render text: 'Ruby!', content_type: 'text/plain'

# good - short and precise
render plain: 'Ruby!'

# good - explicit MIME type not to `text/plain`
render text: 'Ruby!', content_type: 'text/html'
```

With `ContentTypeCompatibility: true` (default), `render text: 'Ruby!'` is left alone because it sets the MIME type to `text/html`; with `false` it is flagged too.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| ContentTypeCompatibility | true |  | Convert only when `content_type` is explicitly set to `text/plain`. |

## Blind spots

None recorded.
