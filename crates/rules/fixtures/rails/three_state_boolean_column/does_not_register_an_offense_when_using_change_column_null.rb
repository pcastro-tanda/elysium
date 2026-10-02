def change
  add_column :users, :active, :boolean
  change_column_null :users, :active, false
end
