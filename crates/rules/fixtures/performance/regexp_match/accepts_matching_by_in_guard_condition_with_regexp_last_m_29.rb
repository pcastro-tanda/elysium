def foo
  return if FOO =~ re

  do_something(::Regexp.last_match)
end
