def change
  add_column :users, :active, :boolean
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Boolean columns should always have a default value and a `NOT NULL` constraint.
  change_column_null :users, :admin, false
  change_column_null :projects, :active, false
  change_column_null :users, :active, true
end
