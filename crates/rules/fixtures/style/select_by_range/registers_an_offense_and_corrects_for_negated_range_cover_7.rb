array.find_all { !(1..10).cover?(_1) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `find_all` with a range check.
