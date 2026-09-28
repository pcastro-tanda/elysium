def something
  array.each do |item|
    if cond
      something
    else
      something2
      redo
    end
    bar
  end
end
