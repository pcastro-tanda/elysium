array.detect { |x| !(1..10).cover?(x) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v(...).first` to `detect` with a range check.
