def change
  add_column :users, :name, :string, null: false
  remove_column :users, :nickname
end
