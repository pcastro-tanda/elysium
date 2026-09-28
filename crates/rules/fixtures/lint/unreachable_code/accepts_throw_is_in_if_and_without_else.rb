def something
  array.each do |item|
    if cond
      something
      throw
    end
    bar
  end
end
