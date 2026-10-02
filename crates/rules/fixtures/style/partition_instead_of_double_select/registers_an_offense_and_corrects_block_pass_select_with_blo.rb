positives = arr.select(&:positive?)
negatives = arr.reject { |x| x.positive? }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `select` and `reject` calls.
