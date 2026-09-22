def my_func
  puts 'do something error prone'
rescue SomeException
  puts 'wrongly indented error handling'
rescue
  puts 'wrongly indented error handling'
end
