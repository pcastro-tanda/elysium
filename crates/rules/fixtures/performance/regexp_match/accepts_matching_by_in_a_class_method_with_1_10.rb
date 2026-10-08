def self.foo
  if FOO =~ re
    do_something($1)
  end
end
