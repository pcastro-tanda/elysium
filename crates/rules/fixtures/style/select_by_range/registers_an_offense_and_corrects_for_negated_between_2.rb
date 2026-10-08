array.select { !it.between?(1, 10) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `select` with a range check.
