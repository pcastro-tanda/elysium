def something
  array.each do |item|
    case cond
    in 1
      something
      return
    in 2
      something2
      return
    end
    bar
  end
end
