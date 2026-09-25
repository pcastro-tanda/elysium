1.times do
  foo = 1
  puts foo
  instance = Object.new
  class << instance
    2
    bar = 3
    puts bar
  end
end
