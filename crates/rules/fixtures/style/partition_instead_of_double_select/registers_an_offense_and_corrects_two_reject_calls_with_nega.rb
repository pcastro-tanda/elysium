a = arr.reject { |x| x.positive? }
b = arr.reject { |x| !x.positive? }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `reject` and `reject` calls.
