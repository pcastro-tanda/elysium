def change
  change_table :users do |t|
  ^^^^^^^^^^^^^^^^^^^ You can combine alter queries using `bulk: true` options.
    t.index :name
    t.index :address
  end
end
