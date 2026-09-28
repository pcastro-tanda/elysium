def something
  array.each do |item|
    case cond
    in 1
      something
      abort
    in 2
      something2
      abort
    end
    bar
  end
end
