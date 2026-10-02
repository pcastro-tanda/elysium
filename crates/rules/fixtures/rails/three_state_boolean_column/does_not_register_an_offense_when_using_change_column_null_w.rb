def change
  add_column table, :active, :boolean
  change_column_null table, :active, false
end
