array_like.each do |n|
  puts 'do something error prone'
rescue SomeException
  puts 'error handling'
rescue
  puts 'error handling'
  else
  ^^^^ Align `else` with `array_like.each`.
  puts 'normal handling'
end
