def something
  array.each do |item|
    abort if cond
    bar
  end
end
