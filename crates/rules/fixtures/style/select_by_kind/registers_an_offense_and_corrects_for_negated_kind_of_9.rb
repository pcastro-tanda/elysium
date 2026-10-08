array.find_all { |x| !x.kind_of?(Foo) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `find_all` with a kind check.
