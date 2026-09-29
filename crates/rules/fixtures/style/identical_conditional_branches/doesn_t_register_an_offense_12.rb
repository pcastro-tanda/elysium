def foo
  case something
  in :a
    do_x
  in :b
    do_x
    x2
  else
    do_x
    x3
  end
end

def bar
  y = case something
      in :a
        do_x
      in :b
        do_x
        x2
      else
        do_x
        x3
      end
  do_something
end
