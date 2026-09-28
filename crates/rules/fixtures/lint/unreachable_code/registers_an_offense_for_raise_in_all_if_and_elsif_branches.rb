def something
  array.each do |item|
    if cond
      something
      raise
    elsif cond2
      something2
      raise
    else
      something3
      raise
    end
    bar
    ^^^ Unreachable code detected.
  end
end
