def something
  array.each do |item|
    if cond
      something
    else
      something2
      exit!
    end
    bar
  end
end
