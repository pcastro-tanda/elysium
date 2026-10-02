hash.fetch('foo', nil)&.fetch(:bar, nil)&.fetch(baz, nil)
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `dig('foo', :bar, baz)` instead.
