def something
  array.each do |item|
    case cond
    when 1
      something
      redo
    when 2
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
