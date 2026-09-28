def something
  array.each do |item|
    case cond
    in 1
      something
      fail
    in 2
      something2
      fail
    end
    bar
  end
end
