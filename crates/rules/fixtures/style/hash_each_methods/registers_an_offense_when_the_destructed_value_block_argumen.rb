foo.each { |k, (_, unused_value)| do_something(k) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `each_key` instead of `each` and remove the unused `(_, unused_value)` block argument.
