def something
  array.each do |item|
    redo if cond
    bar
  end
end
