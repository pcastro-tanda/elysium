# Rails/I18nLocaleAssignment

Prefer the usage of `I18n.with_locale` instead of manually updating `I18n.locale` value.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for the use of `I18n.locale=` method.

The `locale` attribute persists for the rest of the Ruby runtime, potentially causing unexpected behavior at a later time. Using `I18n.with_locale` ensures the code passed in the block is the only place `I18n.locale` is affected. It eliminates the possibility of a `locale` sticking around longer than intended.

```ruby
# bad
I18n.locale = :fr

# good
I18n.with_locale(:fr) do
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
