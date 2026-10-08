array.select { !(1..10).cover?(it) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `select` with a range check.
