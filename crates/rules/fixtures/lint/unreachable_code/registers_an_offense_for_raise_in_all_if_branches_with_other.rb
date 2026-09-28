def something
  array.each do |item|
    if cond
      something
      raise
    else
      something2
      raise
    end
    bar
    ^^^ Unreachable code detected.
  end
end
