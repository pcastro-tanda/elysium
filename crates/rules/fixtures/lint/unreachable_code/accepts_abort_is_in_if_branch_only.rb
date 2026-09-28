def something
  array.each do |item|
    if cond
      something
      abort
    else
      something2
    end
    bar
  end
end
