def self.foo
  if re =~ :foo
    do_something($')
  end
end
