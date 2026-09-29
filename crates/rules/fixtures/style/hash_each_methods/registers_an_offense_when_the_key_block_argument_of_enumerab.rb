foo.each { |unused_key, v| do_something(v) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `each_value` instead of `each` and remove the unused `unused_key` block argument.
