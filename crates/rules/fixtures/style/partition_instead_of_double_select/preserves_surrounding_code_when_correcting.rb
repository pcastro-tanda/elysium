before_code
positives = arr.select { |x| x > 0 }
negatives = arr.reject { |x| x > 0 }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `select` and `reject` calls.
after_code
