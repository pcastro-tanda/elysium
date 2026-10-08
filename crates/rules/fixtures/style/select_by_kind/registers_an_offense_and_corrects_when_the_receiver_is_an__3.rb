[].filter { |x| x.is_a?(Foo) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `filter` with a kind check.
foo.to_a.filter { |x| x.is_a?(Foo) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `filter` with a kind check.
