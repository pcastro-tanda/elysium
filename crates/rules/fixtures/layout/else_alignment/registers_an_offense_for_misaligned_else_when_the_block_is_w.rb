foo << array_like.map do |n|
  puts 'do something error prone'
rescue SomeException
  puts 'error handling'
  else
  ^^^^ Align `else` with `foo`.
  puts 'normal handling'
end
