# Lint/ErbNewArguments

Emulates Ruby 2.6's `ERB.new` argument deprecation warnings.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Now non-keyword arguments other than the first one are softly deprecated
and will be removed when Ruby 2.5 becomes EOL. `ERB.new` with non-keyword
arguments is deprecated since ERB 2.2.0. Use `:trim_mode` and `:eoutvar`
keyword arguments to `ERB.new`. This cop identifies places where
`ERB.new(str, trim_mode, eoutvar)` can be replaced by
`ERB.new(str, trim_mode: trim_mode, eoutvar: eoutvar)`.

```ruby
# bad
ERB.new(str, nil, '-', '@output_buffer')

# good
ERB.new(str, trim_mode: '-', eoutvar: '@output_buffer')
```

## Options

This rule has no options.

## Blind spots

Only a bare `ERB.new`/`::ERB.new` receiver is recognized, matching
upstream's own `(const {nil? cbase} :ERB) :new` node pattern; a receiver
reached through an intermediate constant or variable (e.g. an
`ActionView::Template::Handlers::ERB` alias assigned to a local) is not.
