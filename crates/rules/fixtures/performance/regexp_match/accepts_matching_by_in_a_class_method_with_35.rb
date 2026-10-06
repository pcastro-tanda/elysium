def self.foo
  if re =~ FOO
    do_something($`)
  end
end
