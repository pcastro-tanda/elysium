# Rails/UniqueValidationWithoutIndex

Uniqueness validation should have a unique index on the database column.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for uniqueness validations of columns that have no unique index in `db/schema.rb`.

Validating uniqueness in the application alone is racy; the database should enforce it with a unique index. The cop reads the schema the working directory's `db/schema.rb` dumps and does nothing without one.

```ruby
# bad - rails_helper.rb is the only guard
class Account < ApplicationRecord
  validates :email, uniqueness: true
end

# good - with `t.index ["email"], unique: true` in db/schema.rb
class Account < ApplicationRecord
  validates :email, uniqueness: true
end
```

## Options

This rule has no options.

## Blind spots

The schema is read from the nearest `db/schema.rb` above the working directory when the run starts; `db/structure.sql` is not read.
