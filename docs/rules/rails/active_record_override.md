# Rails/ActiveRecordOverride

Check for overriding Active Record methods instead of using callbacks.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for overriding built-in Active Record methods instead of using callbacks.

```ruby
# bad
class Book < ApplicationRecord
  def save
    self.title = title.upcase!
    super
  end
end

# good
class Book < ApplicationRecord
  before_save :upcase_title

  def upcase_title
    self.title = title.upcase!
  end
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
