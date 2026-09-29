[].each do |o|
  puts o
  unless o == 1
  ^^^^^^^^^^^^^ Use `next` to skip iteration.
    puts o
  end
end
