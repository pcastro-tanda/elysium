positives = arr.find_all { |x| x > 0 }
negatives = arr.reject { |x| x > 0 }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `find_all` and `reject` calls.
