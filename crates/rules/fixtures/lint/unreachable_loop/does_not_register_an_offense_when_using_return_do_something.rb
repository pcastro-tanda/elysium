[nil, nil, 42].each do |value|
  return do_something(value) || next
end
