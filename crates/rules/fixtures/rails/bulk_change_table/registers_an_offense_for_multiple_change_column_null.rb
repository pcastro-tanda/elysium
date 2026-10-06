def change
  change_column_null :users, :name, false
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ You can use `change_table :users, bulk: true` to combine alter queries.
  change_column_null :users, :address, false
end
