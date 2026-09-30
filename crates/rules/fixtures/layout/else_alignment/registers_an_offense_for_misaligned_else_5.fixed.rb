class << self
  puts 'do something error prone'
rescue SomeException
  puts 'error handling'
else
  puts 'normal handling'
ensure
  puts 'cleanup'
end
