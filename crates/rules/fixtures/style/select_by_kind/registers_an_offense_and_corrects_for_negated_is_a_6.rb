array.select { !_1.is_a?(Foo) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `select` with a kind check.
