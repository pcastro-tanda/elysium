def change
  add_reference :users, :team
  add_column :users, :name, :string, null: false
  remove_column :teams, :owner_name
end
