def change
  remove_index :users, :name
  remove_index :users, :address
end
