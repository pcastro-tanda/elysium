loop do
  {}.map do |k, v|
    unless v == 1
    ^^^^^^^^^^^^^ Use `next` to skip iteration.
      puts k
    end
  end
end
