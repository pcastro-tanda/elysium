def foo?
  case condition
  when foo
    !!foo
    ^ Avoid the use of double negation (`!!`).
    do_something
  when bar
    !!bar
    ^ Avoid the use of double negation (`!!`).
    do_something
  else
    !!baz
    ^ Avoid the use of double negation (`!!`).
    do_something
  end
end
