array.detect { |x| (1..10).cover?(x) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep(...).first` to `detect` with a range check.
