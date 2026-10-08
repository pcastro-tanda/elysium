def foo
  bar do
    if /re/.match(foo, 1)
      do_something
    end
  end
  puts $2
end
