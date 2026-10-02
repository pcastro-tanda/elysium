def change
  change_column_default :users, :name, false
  change_column_default :users, :address, false
end
