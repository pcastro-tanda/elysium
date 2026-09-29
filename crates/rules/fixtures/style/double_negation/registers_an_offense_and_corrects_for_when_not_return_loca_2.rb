def foo?
  if condition_foo?
    !!foo
    ^ Avoid the use of double negation (`!!`).
    do_something
  elsif condition_bar?
    !!bar
    ^ Avoid the use of double negation (`!!`).
    do_something
  else
    !!baz
    ^ Avoid the use of double negation (`!!`).
    do_something
  end
end
