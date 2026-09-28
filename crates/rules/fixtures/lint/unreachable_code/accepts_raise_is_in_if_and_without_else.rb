def something
  array.each do |item|
    if cond
      something
      raise
    end
    bar
  end
end
