positives = arr.select { _1 > 0 }
negatives = arr.reject { _1 > 0 }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `select` and `reject` calls.
