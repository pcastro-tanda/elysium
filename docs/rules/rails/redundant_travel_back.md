# Rails/RedundantTravelBack

Checks for redundant `travel_back` calls.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for redundant `travel_back` calls.
Since Rails 5.2, `travel_back` is automatically called at the end of the test.

```ruby
# bad
def teardown
  do_something
  travel_back
end

# good
def teardown
  do_something
end

# bad
after do
  do_something
  travel_back
end

# good
after do
  do_something
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
