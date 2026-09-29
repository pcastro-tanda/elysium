def foo?
  unless condition_foo?
    !!foo
    ^ Avoid the use of double negation (`!!`).
    do_something
  end
end
