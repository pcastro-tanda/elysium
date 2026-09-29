def foo?
  if condition_foo?
    [!!foo0, bar0, baz0]
  elsif condition_bar?
    [!!foo1, bar1, baz1]
  else
    [!!foo2, bar2, baz2]
  end
end

def bar?
  if condition_foo?
    do_something
    [!!foo0, bar0, baz0]
  elsif condition_bar?
    do_something
    [!!foo1, bar1, baz1]
  else
    do_something
    [!!foo2, bar2, baz2]
  end
end
