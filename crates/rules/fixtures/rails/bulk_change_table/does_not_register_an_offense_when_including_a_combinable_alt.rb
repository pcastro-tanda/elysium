def change
  add_reference :users, :team
  add_column :users, :name, :string, null: false
end
