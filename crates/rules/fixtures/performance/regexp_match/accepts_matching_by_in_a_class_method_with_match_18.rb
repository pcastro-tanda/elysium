def self.foo
  if re !~ FOO
    do_something($MATCH)
  end
end
