def change
  change_table :users do |t|
  ^^^^^^^^^^^^^^^^^^^ You can combine alter queries using `bulk: true` options.
    t.string :name, null: false
    t.string :address, null: true
  end
end
