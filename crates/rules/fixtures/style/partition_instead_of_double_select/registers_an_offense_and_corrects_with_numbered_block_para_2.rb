a = arr.select { _1.positive? }
b = arr.select { !_1.positive? }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `select` and `select` calls.
