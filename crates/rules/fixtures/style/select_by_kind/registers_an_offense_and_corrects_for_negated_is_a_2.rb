array.select { !it.is_a?(Foo) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `select` with a kind check.
