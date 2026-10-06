# Rails/MultipleRoutePaths

Checks for mapping a route with multiple paths, which is deprecated and will be removed in Rails 8.1.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for mapping a route with multiple paths, which is deprecated and will be removed in Rails 8.1.

```ruby
# bad
get '/users', '/other_path', to: 'users#index'

# good
get '/users', to: 'users#index'
get '/other_path', to: 'users#index'
```

## Options

This rule has no options.

## Blind spots

None recorded.
