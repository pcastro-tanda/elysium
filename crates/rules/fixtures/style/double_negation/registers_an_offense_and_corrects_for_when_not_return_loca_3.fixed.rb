def foo?
  case condition
  when foo
    !foo.nil?
    do_something
  when bar
    !bar.nil?
    do_something
  else
    !baz.nil?
    do_something
  end
end
