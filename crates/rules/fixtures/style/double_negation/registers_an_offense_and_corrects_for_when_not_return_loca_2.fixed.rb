def foo?
  if condition_foo?
    !foo.nil?
    do_something
  elsif condition_bar?
    !bar.nil?
    do_something
  else
    !baz.nil?
    do_something
  end
end
