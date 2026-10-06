# Rails/Exit

Favor `fail`, `break`, `return`, etc. over `exit` in application or library code outside of Rake files to avoid exits during unit testing or running in production.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Enforces that `exit` and `abort` calls are not used within a rails app. Valid options are instead to raise an error, break, return, or some other form of stopping execution of current request.

There are two obvious cases where `exit` is particularly harmful:

* Usage in library code for your application. Even though Rails will rescue from a `SystemExit` and continue on, unit testing that library code will result in specs exiting (potentially silently if `exit(0)` is used.)
* Usage in application code outside of the web process could result in the program exiting, which could result in the code failing to run and do its job.

```ruby
# bad
exit(0)

# good
raise 'a bad error has happened'
```

## Options

This rule has no options.

## Blind spots

None recorded.
