foo.each { |(_, unused_key), v| do_something(v) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `each_value` instead of `each` and remove the unused `(_, unused_key)` block argument.
