array.filter { !it.kind_of?(Foo) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `filter` with a kind check.
