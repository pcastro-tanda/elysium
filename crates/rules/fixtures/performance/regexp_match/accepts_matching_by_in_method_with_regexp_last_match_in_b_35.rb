def foo
  bar do
    if re !~ FOO
      do_something
    end
  end
  puts Regexp.last_match
end
