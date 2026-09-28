def something
  array.each do |item|
    if cond
      something
      next
    elsif cond2
      something2
    else
      something3
      next
    end
    bar
  end
end
