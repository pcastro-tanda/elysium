def foo
  bar do
    if /re/.match(foo)
      do_something
    end
  end
  puts $1
end
