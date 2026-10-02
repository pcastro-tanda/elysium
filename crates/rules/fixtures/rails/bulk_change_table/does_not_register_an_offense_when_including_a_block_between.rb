def change
  add_column :users, :name, :string, null: false
  User.find_each do |user|
    user.update(name: user.nickname)
  end
  remove_column :users, :nickname
end
