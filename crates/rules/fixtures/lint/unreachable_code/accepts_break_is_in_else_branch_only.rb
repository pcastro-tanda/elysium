def something
  array.each do |item|
    if cond
      something
    else
      something2
      break
    end
    bar
  end
end
