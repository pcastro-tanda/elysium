array.find_all { !it.between?(1, 10) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `find_all` with a range check.
