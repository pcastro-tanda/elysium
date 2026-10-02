def change
  change_table :users, bulk: false do |t|
    t.string :name, null: false
    t.string :address, null: true
  end
end
