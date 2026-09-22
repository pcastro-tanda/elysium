def my_func
  puts 'do something error prone'
rescue SomeException
 puts 'wrongly indented error handling'
^ Use 2 (not 1) spaces for indentation.
rescue
 puts 'wrongly indented error handling'
^ Use 2 (not 1) spaces for indentation.
end
