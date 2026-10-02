def change
  create_table(table) do |t|
    t.boolean :active
  end
  change_column_null table, :active, false
end
