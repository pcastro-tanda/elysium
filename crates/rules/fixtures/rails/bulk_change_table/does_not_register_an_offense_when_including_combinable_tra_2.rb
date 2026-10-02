def change
  change_table :users do |t|
    t.index :name
    t.index :address
  end
end
