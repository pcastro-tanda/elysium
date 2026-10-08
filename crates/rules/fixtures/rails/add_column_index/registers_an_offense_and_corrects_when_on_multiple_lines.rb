add_column :table, :column, :integer,
           index: true,
           ^^^^^^^^^^^ `add_column` does not accept an `index` key, use `add_index` instead.
           default: 0
