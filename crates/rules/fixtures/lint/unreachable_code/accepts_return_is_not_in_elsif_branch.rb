def something
  array.each do |item|
    if cond
      something
      return
    elsif cond2
      something2
    else
      something3
      return
    end
    bar
  end
end
