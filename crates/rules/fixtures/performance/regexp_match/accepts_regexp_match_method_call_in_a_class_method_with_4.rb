def self.foo
  if /re/.match(foo)
    do_something($~)
  end
end
