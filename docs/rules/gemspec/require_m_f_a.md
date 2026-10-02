# Gemspec/RequireMFA

Requires a gemspec to have `rubygems_mfa_required` metadata set.

| | |
| --- | --- |
| Department | Gemspec |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Requires a gemspec to have `rubygems_mfa_required` metadata set.

This setting tells RubyGems that MFA (Multi-Factor Authentication) is
required for accounts to be able to perform privileged operations, such
as `gem push`, `gem yank`, and adding or removing owners.

This helps make your gem more secure, as users can be more confident
that gem updates were pushed by maintainers.

```ruby
# bad
Gem::Specification.new do |spec|
  # no `rubygems_mfa_required` metadata specified
end

# good
Gem::Specification.new do |spec|
  spec.metadata = {
    'rubygems_mfa_required' => 'true'
  }
end

# good
Gem::Specification.new do |spec|
  spec.metadata['rubygems_mfa_required'] = 'true'
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
