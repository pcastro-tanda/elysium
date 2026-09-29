[1, 2, 3].each do |n|
  puts n
rescue StandardError
  handle
end
