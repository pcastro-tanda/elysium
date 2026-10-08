def foo
  bar do
    if foo.match(/re/)
      do_something
    end
  end
  puts Regexp.last_match
end
