array.detect { |x| x.between?(1, 10) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep(...).first` to `detect` with a range check.
