def something
  array.each do |item|
    if cond
      something
    else
      something2
      throw
    end
    bar
  end
end
