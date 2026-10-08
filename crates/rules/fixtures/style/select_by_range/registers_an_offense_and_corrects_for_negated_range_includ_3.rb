array.find_all { |x| !(1..10).include?(x) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `find_all` with a range check.
