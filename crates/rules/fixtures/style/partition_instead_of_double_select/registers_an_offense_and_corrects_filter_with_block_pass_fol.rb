positives = arr.filter(&:positive?)
negatives = arr.reject(&:positive?)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `filter` and `reject` calls.
