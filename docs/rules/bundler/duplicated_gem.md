# Bundler/DuplicatedGem

Checks for duplicate gem entries in Gemfile.

| | |
| --- | --- |
| Department | Bundler |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

A Gem's requirements should be listed only once in a Gemfile.

```ruby
# bad
gem 'rubocop'
gem 'rubocop'

# bad
group :development do
  gem 'rubocop'
end

group :test do
  gem 'rubocop'
end

# good
group :development, :test do
  gem 'rubocop'
end

# good
gem 'rubocop', groups: [:development, :test]

# good - conditional declaration
if Dir.exist?(local)
  gem 'rubocop', path: local
elsif ENV['RUBOCOP_VERSION'] == 'master'
  gem 'rubocop', git: 'https://github.com/rubocop/rubocop.git'
else
  gem 'rubocop', '~> 0.90.0'
end
```

## Options

This rule has no options.

## Blind spots

Only recognizes a plain string literal as the gem name, matching upstream's
`str` node pattern: `gem name_variable` or `gem "#{prefix}-rubocop"` never
groups with anything, even another identical interpolation.
