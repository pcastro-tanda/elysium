def something
  array.each do |item|
    if cond
      something
    else
      something2
      next
    end
    bar
  end
end
