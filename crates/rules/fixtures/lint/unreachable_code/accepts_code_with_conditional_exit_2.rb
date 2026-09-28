def something
  array.each do |item|
    exit! if cond
    bar
  end
end
