def something
  array.each do |item|
    case cond
    in 1
      something
      exit
    in 2
      something2
      exit
    else
      something3
      exit
    end
    bar
    ^^^ Unreachable code detected.
  end
end
