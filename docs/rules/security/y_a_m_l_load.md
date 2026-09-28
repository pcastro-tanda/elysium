# Security/YAMLLoad

Prefer usage of `YAML.safe_load` over `YAML.load` due to potential security issues. See reference for more information.

| | |
| --- | --- |
| Department | Security |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for the use of YAML class methods which have
potential security issues leading to remote code execution when
loading from an untrusted source.

NOTE: Ruby 3.1+ (Psych 4) uses `Psych.load` as `Psych.safe_load` by default.

```ruby
# bad
YAML.load("--- !ruby/object:Foo {}") # Psych 3 is unsafe by default

# good
YAML.safe_load("--- !ruby/object:Foo {}", [Foo])                    # Ruby 2.5  (Psych 3)
YAML.safe_load("--- !ruby/object:Foo {}", permitted_classes: [Foo]) # Ruby 3.0- (Psych 3)
YAML.load("--- !ruby/object:Foo {}", permitted_classes: [Foo])      # Ruby 3.1+ (Psych 4)
YAML.dump(foo)
```

## Options

This rule has no options.

## Blind spots

None recorded.
