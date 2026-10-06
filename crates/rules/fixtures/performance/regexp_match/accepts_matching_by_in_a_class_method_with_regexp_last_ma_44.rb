def self.foo
  if :foo !~ re
    do_something(::Regexp.last_match)
  end
end
