module MyModule
  puts 'do something error prone'
rescue SomeException
  puts 'error handling'
  else
  ^^^^ Align `else` with `module`.
  puts 'normal handling'
end
