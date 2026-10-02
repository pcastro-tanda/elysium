hash.fetch('foo', Hash.new)&.fetch(:bar, Hash.new)&.fetch(baz, nil)
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `dig('foo', :bar, baz)` instead.
