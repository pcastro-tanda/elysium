# Rails/TimeZone

Checks the correct usage of time zone aware methods.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for the use of Time methods without zone.

Built on top of Ruby on Rails style guide (https://rails.rubystyle.guide#time)
and the article http://danilenko.org/2012/7/6/rails_timezones/

Two styles are supported for this cop. When `EnforcedStyle` is 'strict'
then only use of `Time.zone` is allowed.

When EnforcedStyle is 'flexible' then it's also allowed
to use `Time#in_time_zone`.

This cop's autocorrection is unsafe because it may change handling time.

```ruby
# bad
Time.now
Time.parse('2015-03-02T19:05:37')
'2015-03-02T19:05:37'.to_time

# good
Time.current
Time.zone.now
Time.zone.parse('2015-03-02T19:05:37')
Time.zone.parse('2015-03-02T19:05:37Z') # Respect ISO 8601 format with timezone specifier.
Time.parse('2015-03-02T19:05:37Z') # Also respects ISO 8601
'2015-03-02T19:05:37Z'.to_time # Also respects ISO 8601
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `flexible` | `strict`, `flexible` | `strict` means that `Time` should be used with `zone`; `flexible` also allows `in_time_zone`. |

## Blind spots

None recorded.
