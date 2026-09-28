def something
  array.each do |item|
    next if cond
    bar
  end
end
