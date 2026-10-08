# Rails/ContentTag

Use `tag.something` instead of `tag(:something)`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Use `tag` instead of `content_tag` or `tag` with a name argument: `tag.something` instead of `tag(:something)`.

```ruby
# bad
tag(:p, 'Hello world!')
tag(:br, class: 'strong')

# good
tag.p('Hello world!')
tag.br(class: 'strong')
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
