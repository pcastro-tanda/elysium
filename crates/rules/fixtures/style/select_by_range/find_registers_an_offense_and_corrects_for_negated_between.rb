array.find { |x| !x.between?(1, 10) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v(...).first` to `find` with a range check.
