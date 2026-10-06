def self.foo
  if foo !~ /re/
    do_something($`)
  end
end
