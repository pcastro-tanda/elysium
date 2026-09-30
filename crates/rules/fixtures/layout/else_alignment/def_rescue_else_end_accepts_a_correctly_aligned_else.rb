def my_func
  puts 'do something error prone'
rescue SomeException
  puts 'error handling'
rescue
  puts 'error handling'
else
  puts 'normal handling'
end
