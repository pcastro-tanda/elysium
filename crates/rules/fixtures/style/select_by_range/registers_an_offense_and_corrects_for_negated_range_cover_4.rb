array.filter { !(1..10).cover?(it) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `filter` with a range check.
