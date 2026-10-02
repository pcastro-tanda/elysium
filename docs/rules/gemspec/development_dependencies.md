# Gemspec/DevelopmentDependencies

Specify development dependencies in Gemfile.

| | |
| --- | --- |
| Department | Gemspec |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Enforce that development dependencies for a gem are specified in
`Gemfile`, rather than in the `gemspec` using
`add_development_dependency`. Alternatively, using `EnforcedStyle:
gemspec`, enforce that all dependencies are specified in `gemspec`,
rather than in `Gemfile`.

```ruby
# EnforcedStyle: Gemfile (default)
# Specify runtime dependencies in your gemspec,
# but all other dependencies in your Gemfile.

# bad
# example.gemspec
s.add_development_dependency "foo"

# good
# Gemfile
gem "foo"

# good
# gems.rb
gem "foo"
```

```ruby
# EnforcedStyle: gemspec
# Specify all dependencies in your gemspec.

# bad
# Gemfile
gem "foo"

# good
# example.gemspec
s.add_development_dependency "foo"
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `Gemfile` | `Gemfile`, `gems.rb`, `gemspec` | The dependency file development dependencies are expected in. |
| AllowedGems | `[]` |  | Gems that can be specified as a development dependency in either file. |

## Blind spots

None recorded.
