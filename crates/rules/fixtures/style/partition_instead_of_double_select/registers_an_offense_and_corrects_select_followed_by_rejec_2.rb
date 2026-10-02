positives = arr.select(&:positive?)
negatives = arr.reject(&:positive?)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `select` and `reject` calls.
