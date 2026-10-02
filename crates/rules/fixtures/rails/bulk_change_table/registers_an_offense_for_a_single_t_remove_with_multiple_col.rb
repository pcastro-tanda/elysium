def change
  change_table :users do |t|
  ^^^^^^^^^^^^^^^^^^^ You can combine alter queries using `bulk: true` options.
    t.remove :name, :metadata
  end
end
