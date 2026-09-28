# Bundler/DuplicatedGroup

Checks for duplicate gem group entries in Gemfile.

| | |
| --- | --- |
| Department | Bundler |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

A Gem group, or a set of groups, should be listed only once in a Gemfile.

For example, if the values of `source`, `git`, `platforms`, or `path`
surrounding `group` are different, no offense will be registered:

```ruby
platforms :ruby do
  group :default do
    gem 'openssl'
  end
end

platforms :jruby do
  group :default do
    gem 'jruby-openssl'
  end
end
```

```ruby
# bad
group :development do
  gem 'rubocop'
end

group :development do
  gem 'rubocop-rails'
end

# bad (same set of groups declared twice)
group :development do
  gem 'rubocop'
end

group :test, :development do
  gem 'rspec'
end

# good
group :development do
  gem 'rubocop'
end

group :development, :test do
  gem 'rspec'
end

# good
gem 'rubocop', groups: [:development, :test]
gem 'rspec', groups: [:development, :test]
```

## Options

This rule has no options.

## Blind spots

`node.arguments.map(&:source)`/`argument.pairs.map(&:source)`'s equality-by-source-text and
`argument.value.to_s`'s literal-value comparisons are ported as direct byte-slice/`String`
comparisons rather than RuboCop's true structural `Node#==`/`Object#==`; every fixture's
arguments are bare symbol/string/splat/keyword literals, for which this is exact. `find_source_key`
only recognizes `source`/`git`/`platforms`/`path` ancestors regardless of receiver, matching
upstream's own `method_name`-only check (so `Foo.git(...) do ... end` counts too, on both sides of
this port).
