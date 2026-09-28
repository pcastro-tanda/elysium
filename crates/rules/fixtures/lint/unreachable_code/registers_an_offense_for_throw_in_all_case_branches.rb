def something
  array.each do |item|
    case cond
    when 1
      something
      throw
    when 2
      something2
      throw
    else
      something3
      throw
    end
    bar
    ^^^ Unreachable code detected.
  end
end
