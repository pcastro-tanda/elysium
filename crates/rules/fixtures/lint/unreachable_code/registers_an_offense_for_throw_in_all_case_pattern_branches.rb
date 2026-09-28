def something
  array.each do |item|
    case cond
    in 1
      something
      throw
    in 2
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
