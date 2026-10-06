def foo
  bar do
    if /re/i === foo
      do_something
    end
  end
  puts $1
end
