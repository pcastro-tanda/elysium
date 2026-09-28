def something
  array.each do |item|
    case cond
    when 1
      something
      next
    when 2
      something2
      next
    else
      something3
      next
    end
    bar
    ^^^ Unreachable code detected.
  end
end
