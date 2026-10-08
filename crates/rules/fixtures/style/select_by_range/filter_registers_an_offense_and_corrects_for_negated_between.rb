array.filter { |x| !x.between?(1, 10) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `filter` with a range check.
