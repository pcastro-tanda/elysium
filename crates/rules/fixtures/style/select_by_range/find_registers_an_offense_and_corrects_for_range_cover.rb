array.find { |x| (1..10).cover?(x) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep(...).first` to `find` with a range check.
