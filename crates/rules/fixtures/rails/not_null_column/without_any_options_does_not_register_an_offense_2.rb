def change
  change_table :users do |t|
    t.string :name
  end
end
