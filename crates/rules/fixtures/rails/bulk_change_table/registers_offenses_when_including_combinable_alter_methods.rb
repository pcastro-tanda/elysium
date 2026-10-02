def change
  add_reference :users, :team
  add_column :users, :name, :string, null: false
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ You can use `change_table :users, bulk: true` to combine alter queries.
  remove_column :users, :nickname
  remove_column :users, :flag
  add_column :teams, :owner_name, :string, null: false
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ You can use `change_table :teams, bulk: true` to combine alter queries.
  add_column :teams, :member_count, :integer, null: false
  User.reset_column_information
  User.all.each do |user|
    user.refresh!
  end
  remove_column :users, :name
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^ You can use `change_table :users, bulk: true` to combine alter queries.
  remove_column :users, :metadata
end
