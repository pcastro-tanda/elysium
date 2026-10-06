def self.foo
  if "foo".match(re)
    do_something($&)
  end
end
