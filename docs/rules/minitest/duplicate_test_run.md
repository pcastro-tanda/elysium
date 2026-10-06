# Minitest/DuplicateTestRun

This cop detects duplicate test runs caused by one test class inheriting from another.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

If a Minitest class inherits from another class, it will also inherit its methods causing Minitest to run the parent's tests methods twice.

This cop detects when there are two tests classes, one inherits from the other, and both have tests methods. This cop will add an offense to the Child class in such a case.

```ruby
# bad
class ParentTest < Minitest::Test
  def test_parent # it will run this test twice.
  end
end

class ChildTest < ParentTest
  def test_child
  end
end

# good
class ParentTest < Minitest::Test
  def test_parent
  end
end

class ChildTest < Minitest::Test
  def test_child
  end
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
