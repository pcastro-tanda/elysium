def my_func(string)
  puts string
rescue => e
  puts e
else
  puts e
ensure
  puts 'I love methods that print'
end
