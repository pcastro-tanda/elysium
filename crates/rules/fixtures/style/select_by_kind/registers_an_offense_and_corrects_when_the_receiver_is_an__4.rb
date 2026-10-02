[].reject { |x| x.is_a?(Foo) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `reject` with a kind check.
foo.to_a.reject { |x| x.is_a?(Foo) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `reject` with a kind check.
