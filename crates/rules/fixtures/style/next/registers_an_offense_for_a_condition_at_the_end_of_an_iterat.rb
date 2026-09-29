[].each do |o|
  puts o
  if o == 1
  ^^^^^^^^^ Use `next` to skip iteration.
    puts o
  end
end
