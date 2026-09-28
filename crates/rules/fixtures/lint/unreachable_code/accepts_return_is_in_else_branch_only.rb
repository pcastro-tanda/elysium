def something
  array.each do |item|
    if cond
      something
    else
      something2
      return
    end
    bar
  end
end
