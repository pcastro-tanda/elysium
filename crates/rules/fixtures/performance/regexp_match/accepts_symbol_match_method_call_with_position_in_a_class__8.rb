def self.foo
  if :foo.match(re, 1)
    do_something($MATCH)
  end
end
