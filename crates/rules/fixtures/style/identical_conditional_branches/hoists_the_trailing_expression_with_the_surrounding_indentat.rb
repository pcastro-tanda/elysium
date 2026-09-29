def foo
  if something
    bar
    do_x
    ^^^^ Move `do_x` out of the conditional.
  else
    baz
    do_x
    ^^^^ Move `do_x` out of the conditional.
  end
end
