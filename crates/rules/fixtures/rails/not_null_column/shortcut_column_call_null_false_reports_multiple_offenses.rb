def change
  change_table :users do |t|
    t.string :name, null: false
                    ^^^^^^^^^^^ Do not add a NOT NULL column without a default value.
    t.string :address, null: false
                       ^^^^^^^^^^^ Do not add a NOT NULL column without a default value.
  end
end
