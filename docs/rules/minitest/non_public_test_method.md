# Minitest/NonPublicTestMethod

Detects non `public` (marked as `private` or `protected`) test methods.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Detects non `public` (marked as `private` or `protected`) test methods. Minitest runs only test methods which are `public`.

```ruby
# bad
class FooTest
  private # or protected
  def test_does_something
    assert_equal 42, do_something
  end
end

# good
class FooTest
  def test_does_something
    assert_equal 42, do_something
  end
end

# good (not a test case name)
class FooTest
  private # or protected
  def does_something
    assert_equal 42, do_something
  end
end

# good (no assertions)
class FooTest
  private # or protected
  def test_does_something
    do_something
  end
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
