loop do
  while true
    unless o == 1
    ^^^^^^^^^^^^^ Use `next` to skip iteration.
      puts o
    end
  end
end
