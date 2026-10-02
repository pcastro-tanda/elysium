positives = arr.select { it > 0 }
negatives = arr.reject { it > 0 }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `select` and `reject` calls.
