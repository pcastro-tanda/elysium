def change
  if Rails.env.test?
    add_reference :users, :team
    add_column :users, :name, :string, null: true
    remove_column :users, :nickname
  else
    add_reference :users, :team
    add_column :users, :name, :string, null: false
    remove_column :users, :nickname
  end
end
