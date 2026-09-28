def something
  array.each do |item|
    if cond
      redo
    else
      redo
    end
    bar
    ^^^ Unreachable code detected.
  end
end
