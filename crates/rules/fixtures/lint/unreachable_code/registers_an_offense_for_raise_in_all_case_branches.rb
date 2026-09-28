def something
  array.each do |item|
    case cond
    when 1
      something
      raise
    when 2
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
