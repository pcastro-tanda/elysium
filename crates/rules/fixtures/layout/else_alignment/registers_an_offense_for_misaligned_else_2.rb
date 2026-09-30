def my_func(string)
  puts string
rescue => e
  puts e
  else
  ^^^^ Align `else` with `def`.
  puts e
ensure
  puts 'I love methods that print'
end
