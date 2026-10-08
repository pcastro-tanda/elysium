def foo
  bar do
    if FOO =~ re
      do_something
    end
  end
  puts Regexp.last_match(1)
end
