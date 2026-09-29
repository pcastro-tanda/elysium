def foo
  case something
  when :a
    do_x
  when :b
    do_x
    x2
  else
    do_x
    x3
  end
end

def bar
  x = case something
      when :a
        do_x
      when :b
        do_x
        x2
      else
        do_x
        x3
      end
  do_something
end
