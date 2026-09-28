# Lint/UriEscapeUnescape

Checks for places where `URI.escape`/`URI.unescape` (and their aliases) can be replaced by more specific, non-obsolete methods.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Identifies places where `URI.escape` can be replaced by `CGI.escape`,
`URI.encode_www_form`, or `URI.encode_www_form_component` depending on your
specific use case. Also this cop identifies places where `URI.unescape` can
be replaced by `CGI.unescape`, `URI.decode_www_form`, or
`URI.decode_www_form_component` depending on your specific use case.

```ruby
# bad
URI.escape('http://example.com')
URI.encode('http://example.com')

# good
CGI.escape('http://example.com')
URI.encode_uri_component(uri) # Since Ruby 3.1
URI.encode_www_form([['example', 'param'], ['lang', 'en']])
URI.encode_www_form(page: 10, locale: 'en')
URI.encode_www_form_component('http://example.com')

# bad
URI.unescape(enc_uri)
URI.decode(enc_uri)

# good
CGI.unescape(enc_uri)
URI.decode_uri_component(uri) # Since Ruby 3.1
URI.decode_www_form(enc_uri)
URI.decode_www_form_component(enc_uri)
```

## Options

This rule has no options.

## Blind spots

Only matches a receiver that is exactly a bare `URI` or top-level `::URI` constant, per upstream's
`(const {nil? cbase} :URI)` pattern; a re-exported or aliased `URI` (e.g. `Foo::URI.escape(...)` or
`my_uri = URI; my_uri.escape(...)`) is not flagged, matching upstream.
