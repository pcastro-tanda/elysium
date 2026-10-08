Hash.new([])
^^^^^^^^^^^^ Do not create a Hash with a mutable default value as the default value can accidentally be changed.
Hash.new Array.new
^^^^^^^^^^^^^^^^^^ Do not create a Hash with a mutable default value as the default value can accidentally be changed.
