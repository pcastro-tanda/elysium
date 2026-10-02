positives = arr.select do |x|
  x > 0
end
negatives = arr.reject do |x|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `select` and `reject` calls.
  x > 0
end
