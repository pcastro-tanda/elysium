dest = []
src.each { dest << _1 * 2 }
^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `map` instead of `each` to map elements into an array.
