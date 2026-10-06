def change
  change_table :users do |t|
    t.references :address, null: false
                           ^^^^^^^^^^^ Do not add a NOT NULL column without a default value.
  end
end
