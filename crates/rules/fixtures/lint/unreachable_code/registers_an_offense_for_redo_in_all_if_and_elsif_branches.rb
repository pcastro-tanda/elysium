def something
  array.each do |item|
    if cond
      something
      redo
    elsif cond2
      something2
      redo
    else
      something3
      redo
    end
    bar
    ^^^ Unreachable code detected.
  end
end
