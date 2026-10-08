negatives = arr.reject { |x| x > 0 }
positives = arr.select { |x| x > 0 }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `reject` and `select` calls.
