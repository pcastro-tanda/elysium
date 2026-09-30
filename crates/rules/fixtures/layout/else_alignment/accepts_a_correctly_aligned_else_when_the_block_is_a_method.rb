foo(array_like.map do |n|
  puts 'do something error prone'
rescue SomeException
  puts 'error handling'
else
  puts 'normal handling'
end)
