positives = arr.filter { |x| x > 0 }
negatives = arr.reject { |x| x > 0 }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `filter` and `reject` calls.
