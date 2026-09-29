def foo?
  case condition
  when foo
    [!!foo0, bar0, baz0]
  when bar
    [!!foo1, bar1, baz1]
  else
    [!!foo2, bar2, baz2]
  end
end

def foo?
  case condition
  when foo
    do_something
    [!!foo0, bar0, baz0]
  when bar
    do_something
    [!!foo1, bar1, baz1]
  else
    do_something
    [!!foo2, bar2, baz2]
  end
end
