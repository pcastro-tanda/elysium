1.times do
  foo = 1
  puts foo
  array_class = Array
  class SomeClass < array_class
    foo = 2
    ^^^ Useless assignment to variable - `foo`.
    bar = 3
    puts bar
  end
end
