[].find_all { |x| x.is_a?(Foo) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `find_all` with a kind check.
foo.to_a.find_all { |x| x.is_a?(Foo) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `find_all` with a kind check.
