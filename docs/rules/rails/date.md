# Rails/Date

Checks the correct usage of date aware methods, such as Date.today, Date.current etc.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for the correct use of Date methods, such as Date.today, Date.current etc.

Using `Date.today` is dangerous, because it doesn't know anything about Rails time zone. You must use `Time.zone.today` instead.

The cop also reports warnings when you are using `to_time` method, because it doesn't know about Rails time zone either.

Two styles are supported for this cop. When `EnforcedStyle` is `strict` then the Date methods `today`, `current`, `yesterday`, and `tomorrow` are prohibited and the usage of both `to_time` and `to_time_in_current_zone` are reported as warning.

When `EnforcedStyle` is `flexible` then only `Date.today` is prohibited.

And you can set a warning for `to_time` with `AllowToTime: false`. `AllowToTime` is `true` by default to prevent false positive on `DateTime` object.

This cop's autocorrection is unsafe because it may change handling time.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `flexible` | `strict`, `flexible` | `strict` also disallows `Date.current` and friends and `to_time`. |
| AllowToTime | true |  | Whether `to_time` is allowed. |

## Blind spots

None recorded.
