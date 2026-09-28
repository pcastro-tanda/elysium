def something
  array.each do |item|
    if cond
      raise
    else
      raise
    end
    bar
    ^^^ Unreachable code detected.
  end
end
