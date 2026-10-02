def change
  change_table :users do |t|
    t.references :address
  end
end
