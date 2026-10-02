add_column :table, :column, :integer, default: 0
add_index :table, :column, unique: true, name: 'my_unique_index'
