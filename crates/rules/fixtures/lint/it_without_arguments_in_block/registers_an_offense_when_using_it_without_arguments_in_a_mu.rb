0.times do
  it
  ^^ `it` calls without arguments will refer to the first block param in Ruby 3.4; use `it()` or `self.it`.
  it = 1
  it
end
