# Rails/ShortI18n

Use the short form of the I18n methods: `t` instead of `translate` and `l` instead of `localize`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces that short forms of `I18n` methods are used: `t` instead of `translate` and `l` instead of `localize`.

This cop has two different enforcement modes. When the EnforcedStyle is conservative (the default) then only `I18n.translate` and `I18n.localize` calls are added as offenses.

When the EnforcedStyle is aggressive then all `translate` and `localize` calls without a receiver are added as offenses.

```ruby
# bad
I18n.translate :key
I18n.localize Time.now

# good
I18n.t :key
I18n.l Time.now
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `conservative` | `conservative`, `aggressive` | `aggressive` also flags receiverless `translate` and `localize`. |

## Blind spots

None recorded.
