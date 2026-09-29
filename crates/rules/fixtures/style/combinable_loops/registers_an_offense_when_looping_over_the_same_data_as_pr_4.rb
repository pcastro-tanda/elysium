items.each { do_something(_1) }
items.each { do_something_else(_1, arg) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Combine this loop with the previous loop.

items.each_with_index { do_something(_1) }
items.each_with_index { do_something_else(_1, arg) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Combine this loop with the previous loop.

items.reverse_each { do_something(_1) }
items.reverse_each { do_something_else(_1, arg) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Combine this loop with the previous loop.
