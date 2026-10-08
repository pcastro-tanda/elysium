def foo
  bar do
    if foo !~ /re/
      do_something
    end
  end
  puts $2
end
