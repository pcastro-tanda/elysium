def foo?
  case condition
  when foo
    !!foo
  when bar
    !!bar
  else
    !!baz
  end
end

def bar?
  case condition
  when foo
    do_something
    !!foo
  when bar
    do_something
    !!bar
  else
    do_something
    !!baz
  end
end
