def change
  change_column_null :users, :name, false
  change_column_null :users, :address, false
end
