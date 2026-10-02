def change
  add_column :users, :name, :string, null: false
  add_reference :users, :team
  remove_column :users, :nickname
end
