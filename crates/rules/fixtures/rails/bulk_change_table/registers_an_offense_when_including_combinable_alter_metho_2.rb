def change
  remove_index :users, :name
  ^^^^^^^^^^^^^^^^^^^^^^^^^^ You can use `change_table :users, bulk: true` to combine alter queries.
  remove_index :users, :address
end
