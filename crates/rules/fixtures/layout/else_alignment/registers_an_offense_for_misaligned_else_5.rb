class << self
  puts 'do something error prone'
rescue SomeException
  puts 'error handling'
  else
  ^^^^ Align `else` with `class`.
  puts 'normal handling'
ensure
  puts 'cleanup'
end
