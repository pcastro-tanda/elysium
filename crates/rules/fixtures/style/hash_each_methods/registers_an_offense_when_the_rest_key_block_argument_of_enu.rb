foo.each { |*k, v| do_something(*k) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `each_key` instead of `each` and remove the unused `v` block argument.
