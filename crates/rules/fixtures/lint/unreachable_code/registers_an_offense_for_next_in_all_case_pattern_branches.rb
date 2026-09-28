def something
  array.each do |item|
    case cond
    in 1
      something
      next
    in 2
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
