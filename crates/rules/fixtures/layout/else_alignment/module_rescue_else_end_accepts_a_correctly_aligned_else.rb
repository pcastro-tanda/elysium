module MyModule
  puts 'do something error prone'
rescue SomeException
  puts 'error handling'
else
  puts 'normal handling'
end
