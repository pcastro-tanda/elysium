def something
  array.each do |item|
    if cond
      something
      raise
    else
      something2
    end
    bar
  end
end
