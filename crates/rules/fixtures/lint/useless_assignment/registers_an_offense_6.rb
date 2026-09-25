1.times do
  foo = 1
  puts foo
  instance = Object.new
  class << instance
    foo = 2
    ^^^ Useless assignment to variable - `foo`.
    bar = 3
    puts bar
  end
end
