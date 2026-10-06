def foo
  if re =~ :foo
    do_something(Regexp.last_match)
  end
end
