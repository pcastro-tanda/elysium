def something
  array.each do |item|
    if cond
      something
      next
    else
      something2
    end
    bar
  end
end
