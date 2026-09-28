[nil, nil, 42].each do |value|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ This loop will have at most one iteration.
  return do_something(value) || break
end
