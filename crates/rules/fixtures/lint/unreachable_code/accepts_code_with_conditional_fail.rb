def something
  array.each do |item|
    fail if cond
    bar
  end
end
