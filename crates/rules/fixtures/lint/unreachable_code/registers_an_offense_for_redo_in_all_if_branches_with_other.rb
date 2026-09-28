def something
  array.each do |item|
    if cond
      something
      redo
    else
      something2
      redo
    end
    bar
    ^^^ Unreachable code detected.
  end
end
