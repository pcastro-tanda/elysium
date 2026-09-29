def foo
  if something
    do_x
  else
    do_x
    1 + 2 + 3
  end
end

def bar
  y = if something
        do_x
      else
        do_x
        1 + 2 + 3
      end
  do_something_else
end
