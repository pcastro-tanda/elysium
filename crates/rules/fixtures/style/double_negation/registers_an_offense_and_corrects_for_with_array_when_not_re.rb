def foo?
  if condition_foo?
    [!!foo0, bar0, baz0]
     ^ Avoid the use of double negation (`!!`).
    do_something
  elsif condition_bar?
    [!!foo1, bar1, baz1]
     ^ Avoid the use of double negation (`!!`).
    do_something
  else
    [!!foo2, bar2, baz2]
     ^ Avoid the use of double negation (`!!`).
    do_something
  end
end
