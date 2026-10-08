array.find_all { !_1.is_a?(Foo) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `find_all` with a kind check.
