[1, 2, 3, 4].each do |num|
  if !opts.nil?
  ^^^^^^^^^^^^^ Use `next` to skip iteration.
    puts num
    if num != 2
      puts 'hello'
      puts 'world'
    end
 end
end
