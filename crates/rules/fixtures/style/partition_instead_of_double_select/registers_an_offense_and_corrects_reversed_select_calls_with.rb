b = arr.select { |x| !x.positive? }
a = arr.select { |x| x.positive? }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `select` and `select` calls.
