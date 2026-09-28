def something
  array.each do |item|
    if cond
      something
      throw
    else
      something2
    end
    bar
  end
end
