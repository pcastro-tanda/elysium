def change
  change_table :users do |t|
  ^^^^^^^^^^^^^^^^^^^ You can combine alter queries using `bulk: true` options.
    t.change_default :name, 'unknown'
    t.change_default :address, nil
  end
end
