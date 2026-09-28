[1, 2, 3].each do
^^^^^^^^^^^^^^^^^ This loop will have at most one iteration.
  return it.odd? || break
end
