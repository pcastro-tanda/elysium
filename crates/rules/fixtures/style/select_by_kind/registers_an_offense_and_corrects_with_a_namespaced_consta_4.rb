array.reject { |x| !x.is_a?(Foo::Bar) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `reject` with a kind check.
