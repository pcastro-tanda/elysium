def foo?
  case pattern
  in foo
    !!foo
    ^ Avoid the use of double negation (`!!`).
    do_something
  in bar
    !!bar
    ^ Avoid the use of double negation (`!!`).
    do_something
  else
    !!baz
    ^ Avoid the use of double negation (`!!`).
    do_something
  end
end
