def x
  return @x if defined?(@x)
  @x = false
  do_something
end
