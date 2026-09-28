def something
  array.each do |item|
    case cond
    in 1
      something
      raise
    in 2
      something2
      raise
    end
    bar
  end
end
