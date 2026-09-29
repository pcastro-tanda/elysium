foo.each { |k, *v| do_something(*v) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `each_value` instead of `each` and remove the unused `k` block argument.
