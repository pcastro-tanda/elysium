def something
  array.each do |item|
    if cond
      something
      raise
    elsif cond2
      something2
    else
      something3
      raise
    end
    bar
  end
end
