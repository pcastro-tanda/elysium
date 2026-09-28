def something
  array.each do |item|
    case cond
    in 1
      something
      next
    in 2
      something2
      next
    end
    bar
  end
end
