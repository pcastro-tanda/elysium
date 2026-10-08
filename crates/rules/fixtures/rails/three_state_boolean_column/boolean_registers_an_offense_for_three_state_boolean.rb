t.boolean :active
^^^^^^^^^^^^^^^^^ Boolean columns should always have a default value and a `NOT NULL` constraint.
t.boolean :active, default: true
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Boolean columns should always have a default value and a `NOT NULL` constraint.
t.boolean :active, default: true, null: true
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Boolean columns should always have a default value and a `NOT NULL` constraint.
