# Bundler/OrderedGems

Gems within groups in the Gemfile should be alphabetically sorted.

| | |
| --- | --- |
| Department | Bundler |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Gems should be alphabetically sorted within groups.

```ruby
# bad
gem 'rubocop'
gem 'rspec'

# good
gem 'rspec'
gem 'rubocop'

# good
gem 'rubocop'

gem 'rspec'
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| TreatCommentsAsGroupSeparators | true |  | A comment directly above a gem breaks up its group, so gems on either side of it are compared separately. |
| ConsiderPunctuation | false |  | By default, `-` and `_` are ignored for order purposes; set this to compare them too. |

## Blind spots

`Include: ['**/*.gemfile', '**/Gemfile', '**/gems.rb']` restricts this cop to Gemfile-like files
upstream; elysium applies that restriction at the config-file-matching layer (see
`crates/config`), not in this rule's own logic.
