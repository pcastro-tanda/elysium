def foo?
  if condition_foo?
    !!foo
  elsif condition_bar?
    !!bar
  else
    !!baz
  end
end

def bar?
  if condition_foo?
    do_something
    !!foo
  elsif condition_bar?
    do_something
    !!bar
  else
    do_something
    !!baz
  end
end
