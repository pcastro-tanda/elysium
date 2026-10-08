def change
  change_table :users do |t|
    t.column :name, :string, null: false
                             ^^^^^^^^^^^ Do not add a NOT NULL column without a default value.
  end
end
