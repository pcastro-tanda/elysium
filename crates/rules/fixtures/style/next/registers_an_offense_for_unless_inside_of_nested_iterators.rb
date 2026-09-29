loop do
  until false
    unless o == 1
    ^^^^^^^^^^^^^ Use `next` to skip iteration.
      puts o
    end
  end
end
