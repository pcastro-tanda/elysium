array.reject do |x|
^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `reject` with a range check.
  x.between?(1, 10)
end
