def something
  array.each do |item|
    if cond
      something
      exit!
    end
    bar
  end
end
