# Lint/UriRegexp

Identifies places where `URI.regexp` is obsolete and should not be used.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Identifies places where `URI.regexp` is obsolete and should not be used.

For Ruby 3.3 or lower, use `URI::DEFAULT_PARSER.make_regexp`.
For Ruby 3.4 or higher, use `URI::RFC2396_PARSER.make_regexp`.

NOTE: If you need to support both Ruby 3.3 and lower as well as Ruby 3.4 and higher,
consider manually changing the code as follows:

```ruby
defined?(URI::RFC2396_PARSER) ? URI::RFC2396_PARSER : URI::DEFAULT_PARSER
```

```ruby
# bad
URI.regexp('http://example.com')

# good - Ruby 3.3 or lower
URI::DEFAULT_PARSER.make_regexp('http://example.com')

# good - Ruby 3.4 or higher
URI::RFC2396_PARSER.make_regexp('http://example.com')
```

## Options

This rule has no options.

## Blind spots

Only matches a receiver that is exactly a bare `URI` or top-level `::URI` constant, per upstream's
`(const {cbase nil?} :URI)` pattern; a re-exported or aliased `URI` (e.g. `Foo::URI.regexp(...)` or
`my_uri = URI; my_uri.regexp(...)`) is not flagged, matching upstream. Only the first argument's
source is used when rebuilding the replacement call, matching upstream's
`node.first_argument.source` (any further arguments are dropped from the suggested replacement).
