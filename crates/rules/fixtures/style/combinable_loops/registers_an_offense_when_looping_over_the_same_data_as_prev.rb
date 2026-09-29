items.each { |item| do_something(item) }
items.each { |item| do_something_else(item, arg) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Combine this loop with the previous loop.

items.each_with_index { |item| do_something(item) }
items.each_with_index { |item| do_something_else(item, arg) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Combine this loop with the previous loop.

items.reverse_each { |item| do_something(item) }
items.reverse_each { |item| do_something_else(item, arg) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Combine this loop with the previous loop.
