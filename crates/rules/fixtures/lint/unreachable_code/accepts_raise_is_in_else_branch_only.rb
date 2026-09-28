def something
  array.each do |item|
    if cond
      something
    else
      something2
      raise
    end
    bar
  end
end
