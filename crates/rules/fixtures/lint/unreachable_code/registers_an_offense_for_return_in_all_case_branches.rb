def something
  array.each do |item|
    case cond
    when 1
      something
      return
    when 2
      something2
      return
    else
      something3
      return
    end
    bar
    ^^^ Unreachable code detected.
  end
end
