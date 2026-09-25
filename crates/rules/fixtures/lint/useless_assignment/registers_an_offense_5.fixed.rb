1.times do
  foo = 1
  puts foo
  array_class = Array
  class SomeClass < array_class
    2
    bar = 3
    puts bar
  end
end
