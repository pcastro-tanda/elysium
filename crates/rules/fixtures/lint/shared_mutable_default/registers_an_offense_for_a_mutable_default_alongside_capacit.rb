Hash.new([], capacity: 42)
^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not create a Hash with a mutable default value as the default value can accidentally be changed.
Hash.new(Array.new, capacity: 42)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not create a Hash with a mutable default value as the default value can accidentally be changed.
Hash.new(Hash.new, capacity: 42)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not create a Hash with a mutable default value as the default value can accidentally be changed.
