def foo
  bar do
    if :foo.match(re, 1)
      do_something
    end
  end
  puts Regexp.last_match
end
