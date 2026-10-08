# Rails/ReversibleMigration

Checks whether the change method of the migration file is reversible.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks whether the change method of the migration file is reversible.

```ruby
# bad
def change
  change_table :users do |t|
    t.remove :name
  end
end

# good
def change
  change_table :users do |t|
    t.remove :name, type: :string
  end
end

# good
def change
  create_table :users do |t|
    t.string :name
  end
end

# bad
def change
  drop_table :users
end

# good
def change
  drop_table :users do |t|
    t.string :name
  end
end

# bad
def change
  remove_column(:suppliers, :qualification)
end

# good
def change
  remove_column(:suppliers, :qualification, :string)
end
```

## Options

This rule has no options.

## Blind spots

Inside `change_table`, only sends that are statements of the block (or the sole statement's call) are inspected, not those nested in other constructs.
