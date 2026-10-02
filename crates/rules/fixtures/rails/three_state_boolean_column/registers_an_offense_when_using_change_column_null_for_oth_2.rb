def change
  create_table(:users) do |t|
    t.column :active, :boolean
    ^^^^^^^^^^^^^^^^^^^^^^^^^^ Boolean columns should always have a default value and a `NOT NULL` constraint.
  end
  change_column_null :users, :admin, false
  change_column_null :projects, :active, false
  change_column_null :users, :active, true
end
