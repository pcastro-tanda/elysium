add_column :users, :name, :string
User.update_all(name: "dummy")
change_column :users, :name, :string, null: false
