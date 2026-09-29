def x
  do_something
  return @x if defined?(@x)
  @x = false
end
