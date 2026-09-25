1.times do
  foo = 1
  puts foo
  instance = Object.new
  def instance.some_method
    2
    bar = 3
    puts bar
  end
end
