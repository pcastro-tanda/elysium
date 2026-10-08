def self.foo
  if re !~ FOO
    do_something(Regexp.last_match)
  end
end
