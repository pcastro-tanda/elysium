def change
  create_table(:users) do |t|
    t.boolean :active
  end
  change_column_null :users, :active, false
end
