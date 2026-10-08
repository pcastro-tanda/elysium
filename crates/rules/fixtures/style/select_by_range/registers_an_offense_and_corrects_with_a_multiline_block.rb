array.select do |x|
^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `select` with a range check.
  x.between?(1, 10)
end
