def self.foo
  if FOO =~ re
    do_something(Regexp.last_match)
  end
end
