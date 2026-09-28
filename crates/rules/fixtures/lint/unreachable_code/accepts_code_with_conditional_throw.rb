def something
  array.each do |item|
    throw if cond
    bar
  end
end
