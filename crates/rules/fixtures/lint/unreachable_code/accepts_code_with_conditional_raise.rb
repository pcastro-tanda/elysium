def something
  array.each do |item|
    raise if cond
    bar
  end
end
