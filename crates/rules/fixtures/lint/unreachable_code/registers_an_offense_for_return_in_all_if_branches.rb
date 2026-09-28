def something
  array.each do |item|
    if cond
      return
    else
      return
    end
    bar
    ^^^ Unreachable code detected.
  end
end
