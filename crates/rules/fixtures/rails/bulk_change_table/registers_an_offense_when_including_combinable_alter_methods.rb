def change
  add_column :users, :name, :string, null: false
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ You can use `change_table :users, bulk: true` to combine alter queries.
  remove_column :users, :nickname
end
