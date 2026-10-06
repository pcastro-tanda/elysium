def self.foo
  if :foo.match(re)
    do_something(Regexp.last_match(1))
  end
end
