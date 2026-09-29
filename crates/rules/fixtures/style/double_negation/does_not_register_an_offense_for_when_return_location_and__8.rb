def foo?
  case condition
  in foo
    !!foo
  in bar
    !!bar
  else
    !!baz
  end
end

def bar?
  case condition
  in foo
    do_something
    !!foo
  in bar
    do_something
    !!bar
  else
    do_something
    !!baz
  end
end
