[1, 2, 3, 4].each do |num|
  next unless !opts.nil?
  puts num
  next unless num != 2
  puts 'hello'
  puts 'world'
end
