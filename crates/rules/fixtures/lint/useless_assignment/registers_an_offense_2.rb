class SomeClass
  foo = 1
  puts foo
  def self.some_method
    foo = 2
    ^^^ Useless assignment to variable - `foo`.
    bar = 3
    puts bar
  end
end
