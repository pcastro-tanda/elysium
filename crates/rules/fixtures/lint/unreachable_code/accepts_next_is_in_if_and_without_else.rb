def something
  array.each do |item|
    if cond
      something
      next
    end
    bar
  end
end
