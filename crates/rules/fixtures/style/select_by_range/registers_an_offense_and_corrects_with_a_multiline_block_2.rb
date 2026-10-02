array.find_all do |x|
^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `find_all` with a range check.
  x.between?(1, 10)
end
