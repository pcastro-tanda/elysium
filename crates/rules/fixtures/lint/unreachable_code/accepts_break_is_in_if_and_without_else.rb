def something
  array.each do |item|
    if cond
      something
      break
    end
    bar
  end
end
