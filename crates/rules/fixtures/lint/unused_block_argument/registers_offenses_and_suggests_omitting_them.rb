hash = { foo: 'FOO', bar: 'BAR' }
hash.each do |key, value|
                   ^^^^^ Unused block argument - `value`. You can omit all the arguments if you don't care about them.
              ^^^ Unused block argument - `key`. You can omit all the arguments if you don't care about them.
  puts :something
end
