def change
  change_column_default :users, :name, false
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ You can use `change_table :users, bulk: true` to combine alter queries.
  change_column_default :users, :address, false
end
