def change
  create_table(table) do |t|
    t.column :active, :boolean
  end
  change_column_null table, :active, false
end
