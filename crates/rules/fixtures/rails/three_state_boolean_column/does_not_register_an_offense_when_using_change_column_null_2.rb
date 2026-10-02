def change
  create_table(:users) do |t|
    t.column :active, :boolean
  end
  change_column_null :users, :active, false
end
