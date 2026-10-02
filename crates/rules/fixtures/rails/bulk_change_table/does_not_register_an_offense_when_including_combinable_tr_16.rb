def change
  change_table :users do |t|
    t.string :name, null: false
    t.string :address, null: true
  end
end
