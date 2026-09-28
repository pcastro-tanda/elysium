def something
  array.each do |item|
    if cond
      something
      redo
    end
    bar
  end
end
