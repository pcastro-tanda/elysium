def change
  change_table :users, bulk: true do |t|
    t.string :name, null: false
    t.string :address, null: true
  end
end
