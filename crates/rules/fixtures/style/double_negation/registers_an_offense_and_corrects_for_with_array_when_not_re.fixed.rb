def foo?
  if condition_foo?
    [!foo0.nil?, bar0, baz0]
    do_something
  elsif condition_bar?
    [!foo1.nil?, bar1, baz1]
    do_something
  else
    [!foo2.nil?, bar2, baz2]
    do_something
  end
end
