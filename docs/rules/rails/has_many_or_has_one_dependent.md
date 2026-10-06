# Rails/HasManyOrHasOneDependent

Define the dependent option to the has_many and has_one associations.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Looks for `has_many` or `has_one` associations that don't specify a `:dependent` option.

It doesn't register an offense if `:through` or `dependent: nil` is specified, or if the model is read-only.

```ruby
# bad
class User < ActiveRecord::Base
  has_many :comments
  has_one :avatar
end

# good
class User < ActiveRecord::Base
  has_many :comments, dependent: :restrict_with_exception
  has_one :avatar, dependent: :destroy
  has_many :articles, dependent: nil
  has_many :patients, through: :appointments
end

class User < ActiveRecord::Base
  has_many :comments
  has_one :avatar

  def readonly?
    true
  end
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
