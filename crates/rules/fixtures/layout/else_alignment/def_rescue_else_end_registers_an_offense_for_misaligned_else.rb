def my_func
  puts 'do something error prone'
rescue SomeException
  puts 'error handling'
rescue
  puts 'error handling'
  else
  ^^^^ Align `else` with `def`.
  puts 'normal handling'
end
