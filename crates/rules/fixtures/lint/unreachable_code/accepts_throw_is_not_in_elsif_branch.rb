def something
  array.each do |item|
    if cond
      something
      throw
    elsif cond2
      something2
    else
      something3
      throw
    end
    bar
  end
end
