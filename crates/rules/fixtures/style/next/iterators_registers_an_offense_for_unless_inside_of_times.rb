loop do
  3.times do |o|
    unless o == 1
    ^^^^^^^^^^^^^ Use `next` to skip iteration.
      puts o
    end
  end
end
