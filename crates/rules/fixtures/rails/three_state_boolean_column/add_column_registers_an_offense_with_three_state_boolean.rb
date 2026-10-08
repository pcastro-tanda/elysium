add_column :users, :active, :boolean
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Boolean columns should always have a default value and a `NOT NULL` constraint.
add_column :users, :active, :boolean, default: true
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Boolean columns should always have a default value and a `NOT NULL` constraint.
add_column :users, :active, :boolean, default: true, null: true
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Boolean columns should always have a default value and a `NOT NULL` constraint.
