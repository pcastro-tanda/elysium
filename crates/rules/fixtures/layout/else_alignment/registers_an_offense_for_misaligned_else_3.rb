class MyClass
  puts 'do something error prone'
rescue SomeException
  puts 'error handling'
  else
  ^^^^ Align `else` with `class`.
  puts 'normal handling'
end
