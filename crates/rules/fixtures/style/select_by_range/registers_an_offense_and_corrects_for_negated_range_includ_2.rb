array.select { |x| !(1..10).include?(x) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `select` with a range check.
