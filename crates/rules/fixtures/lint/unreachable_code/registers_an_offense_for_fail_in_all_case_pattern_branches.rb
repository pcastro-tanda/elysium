def something
  array.each do |item|
    case cond
    in 1
      something
      fail
    in 2
      something2
      fail
    else
      something3
      fail
    end
    bar
    ^^^ Unreachable code detected.
  end
end
