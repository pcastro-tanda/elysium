def foo
  bar do
    if :foo.match(re)
      do_something
    end
  end
  puts $2
end
