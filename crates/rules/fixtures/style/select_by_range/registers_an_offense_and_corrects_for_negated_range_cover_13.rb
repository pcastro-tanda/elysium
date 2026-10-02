array.find { |x| !(1..10).cover?(x) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v(...).first` to `find` with a range check.
