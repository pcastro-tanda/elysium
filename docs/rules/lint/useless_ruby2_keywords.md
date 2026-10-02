# Lint/UselessRuby2Keywords

Finds unnecessary uses of `ruby2_keywords`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Looks for `ruby2_keywords` calls for methods that do not need it.

`ruby2_keywords` should only be called on methods that accept an argument
splat (`*args`) but do not have explicit keyword arguments (`k:` or `k:
true`) or a keyword splat (`**kwargs`).

```ruby
# good (splat argument without keyword arguments)
ruby2_keywords def foo(*args); end

# bad (no arguments)
ruby2_keywords def foo; end

# bad (positional argument)
ruby2_keywords def foo(arg); end

# bad (double splatted argument)
ruby2_keywords def foo(**args); end

# bad (keyword arguments)
ruby2_keywords def foo(i:, j:); end

# bad (splat argument with keyword arguments)
ruby2_keywords def foo(*args, i:, j:); end

# bad (ruby2_keywords given a symbol)
def foo; end
ruby2_keywords :foo

# bad (ruby2_keywords with dynamic method)
define_method(:foo) { |arg| }
ruby2_keywords :foo
```

## Options

This rule has no options.

## Blind spots

None recorded.
