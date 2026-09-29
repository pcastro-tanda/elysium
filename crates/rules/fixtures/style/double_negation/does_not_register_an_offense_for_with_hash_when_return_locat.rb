def foo?
  if condition_foo?
    { foo: !!foo0, bar: bar0, baz: baz0 }
  elsif condition_bar?
    { foo: !!foo1, bar: bar1, baz: baz1 }
  else
    { foo: !!foo2, bar: bar2, baz: baz2 }
  end
end

def bar?
  if condition_foo?
    do_something
    { foo: !!foo0, bar: bar0, baz: baz0 }
  elsif condition_bar?
    do_something
    { foo: !!foo1, bar: bar1, baz: baz1 }
  else
    do_something
    { foo: !!foo2, bar: bar2, baz: baz2 }
  end
end
