def something
  array.each do |item|
    case cond
    in 1
      something
      break
    in 2
      something2
      break
    else
      something3
      break
    end
    bar
    ^^^ Unreachable code detected.
  end
end
