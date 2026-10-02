def change
  change_table :users do |t|
  ^^^^^^^^^^^^^^^^^^^ You can combine alter queries using `bulk: true` options.
    t.change_null :name, false
    t.change_null :address, false
  end
end
