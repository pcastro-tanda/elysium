array.select { |x| !x.kind_of?(Foo) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `select` with a kind check.
