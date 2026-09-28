# Gemspec/RubyVersionGlobalsUsage

Checks usage of RUBY_VERSION in gemspec.

| | |
| --- | --- |
| Department | Gemspec |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks that `RUBY_VERSION` and `Ruby::VERSION` constants are not used in gemspec.

Using `RUBY_VERSION` and `Ruby::VERSION` are dangerous because value of the constant is
determined by `rake release`. It's possible to have dependency based on ruby version used
to execute `rake release` and not user's ruby version.

```ruby
# bad
Gem::Specification.new do |spec|
  if RUBY_VERSION >= '3.0'
    spec.add_dependency 'gem_a'
  else
    spec.add_dependency 'gem_b'
  end
end

# good
Gem::Specification.new do |spec|
  spec.add_dependency 'gem_a'
end
```

## Options

This rule has no options.

## Blind spots

Upstream's `gem_specification` search does not require the flagged constant to be nested inside
the `Gem::Specification.new` block, only that such a block exists somewhere in the same file; this
port reproduces that literally (see the module doc), which is a real upstream quirk, not a
mistranslation. `Include: ['**/*.gemspec']` restricts this cop to gemspec files upstream; elysium
applies that restriction at the config-file-matching layer (see `crates/config`), not in this
rule's own logic.
