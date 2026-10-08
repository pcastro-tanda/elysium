positives = foo.bar.select { |x| x > 0 }
negatives = foo.bar.reject { |x| x > 0 }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `select` and `reject` calls.
