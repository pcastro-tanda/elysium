def something
  array.each do |item|
    case cond
    in 1
      something
      redo
    in 2
      something2
      redo
    else
      something3
      redo
    end
    bar
    ^^^ Unreachable code detected.
  end
end
