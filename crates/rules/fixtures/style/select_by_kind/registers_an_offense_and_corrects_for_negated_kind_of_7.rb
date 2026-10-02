array.find_all { !_1.kind_of?(Foo) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `find_all` with a kind check.
