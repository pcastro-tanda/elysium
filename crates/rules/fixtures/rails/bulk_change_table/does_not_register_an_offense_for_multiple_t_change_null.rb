def change
  change_table :users do |t|
    t.change_null :name, false
    t.change_null :address, false
  end
end
