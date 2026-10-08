array.filter { !_1.is_a?(Foo) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `filter` with a kind check.
