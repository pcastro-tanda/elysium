def something
  array.each do |item|
    if cond
      something
      return
    else
      something2
      return
    end
    bar
    ^^^ Unreachable code detected.
  end
end
