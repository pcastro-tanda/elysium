array.filter do |x|
^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `filter` with a range check.
  x.between?(1, 10)
end
