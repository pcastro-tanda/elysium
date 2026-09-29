foo(:foo) do |bar: 'default'|
              ^^^ Unused block argument - `bar`. You can omit the argument if you don't care about it.
  puts 'bar'
end
