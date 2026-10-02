hash.fetch('foo', {})&.fetch(:bar, {})&.fetch(baz, nil)
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `dig('foo', :bar, baz)` instead.
