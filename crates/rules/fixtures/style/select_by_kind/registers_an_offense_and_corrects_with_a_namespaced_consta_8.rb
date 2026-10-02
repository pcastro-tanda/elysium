array.reject { |x| x.is_a?(Foo::Bar) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `reject` with a kind check.
