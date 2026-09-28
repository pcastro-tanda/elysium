def something
  array.each do |item|
    if cond
      something
      break
    elsif cond2
      something2
    else
      something3
      break
    end
    bar
  end
end
