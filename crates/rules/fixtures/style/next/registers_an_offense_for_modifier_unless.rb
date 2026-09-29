[].each do |o|
  puts o unless o == 1 # comment
  ^^^^^^^^^^^^^^^^^^^^ Use `next` to skip iteration.
end
