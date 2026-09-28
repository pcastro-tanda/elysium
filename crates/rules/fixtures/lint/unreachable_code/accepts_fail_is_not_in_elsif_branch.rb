def something
  array.each do |item|
    if cond
      something
      fail
    elsif cond2
      something2
    else
      something3
      fail
    end
    bar
  end
end
