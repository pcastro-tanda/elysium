negatives = arr.reject(&:positive?)
positives = arr.select(&:positive?)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `reject` and `select` calls.
