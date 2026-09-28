def something
  array.each do |item|
    if cond
      something
      exit!
    elsif cond2
      something2
    else
      something3
      exit!
    end
    bar
  end
end
