arr.select { |x| x.positive? }
arr.select { |x| !x.positive? }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `select` and `select` calls.
