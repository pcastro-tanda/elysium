def m
  items.something do |i|
    do_something
    yield i
  end
end
