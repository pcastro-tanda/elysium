def something
  array.each do |item|
    return if cond
    bar
  end
end
