def foo
  if :foo.match(re, 1)
    do_something($2)
  end
end
