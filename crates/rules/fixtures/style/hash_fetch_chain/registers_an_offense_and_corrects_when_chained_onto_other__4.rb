x.hash.fetch('foo', ::Hash.new)&.fetch('bar', nil)
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `dig('foo', 'bar')` instead.
