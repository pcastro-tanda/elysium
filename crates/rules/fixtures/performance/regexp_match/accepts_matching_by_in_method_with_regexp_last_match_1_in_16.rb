def foo
  bar do
    if re !~ :foo
      do_something
    end
  end
  puts Regexp.last_match(1)
end
