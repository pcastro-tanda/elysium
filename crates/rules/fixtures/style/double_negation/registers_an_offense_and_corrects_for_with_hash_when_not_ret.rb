def foo?
  if condition_foo?
    { foo: !!foo0, bar: bar0, baz: baz0 }
           ^ Avoid the use of double negation (`!!`).
    do_something
  elsif condition_bar?
    { foo: !!foo1, bar: bar1, baz: baz1 }
           ^ Avoid the use of double negation (`!!`).
    do_something
  else
    { foo: !!foo2, bar: bar2, baz: baz2 }
           ^ Avoid the use of double negation (`!!`).
    do_something
  end
end
