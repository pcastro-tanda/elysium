# Rails/MigrationClassName

The class name of the migration should match its file name.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Makes sure that each migration file defines a migration class whose name matches the file name. (e.g. `20220224111111_create_users.rb` should define `CreateUsers` class.)

```ruby
# db/migrate/20220224111111_create_users.rb

# bad
class SellBooks < ActiveRecord::Migration[7.0]
end

# good
class CreateUsers < ActiveRecord::Migration[7.0]
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
