def foo?
  case pattern
  in foo
    !foo.nil?
    do_something
  in bar
    !bar.nil?
    do_something
  else
    !baz.nil?
    do_something
  end
end
