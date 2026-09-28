def something
  array.each do |item|
    if cond
      something
      return
    else
      something2
    end
    bar
  end
end
