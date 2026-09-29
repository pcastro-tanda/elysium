loop do
  {}.map do |k, v|
    next unless v == 1
    puts k
  end
end
