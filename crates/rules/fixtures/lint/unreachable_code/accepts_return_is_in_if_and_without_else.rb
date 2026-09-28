def something
  array.each do |item|
    if cond
      something
      return
    end
    bar
  end
end
