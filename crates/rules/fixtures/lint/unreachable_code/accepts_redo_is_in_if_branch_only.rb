def something
  array.each do |item|
    if cond
      something
      redo
    else
      something2
    end
    bar
  end
end
