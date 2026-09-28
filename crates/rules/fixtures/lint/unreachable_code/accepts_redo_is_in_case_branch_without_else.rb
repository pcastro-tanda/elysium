def something
  array.each do |item|
    case cond
    when 1
      something
      redo
    when 2
      something2
      redo
    end
    bar
  end
end
