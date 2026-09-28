def something
  array.each do |item|
    case cond
    in 1
      something
      abort
    in 2
      something2
      abort
    else
      something3
      abort
    end
    bar
    ^^^ Unreachable code detected.
  end
end
