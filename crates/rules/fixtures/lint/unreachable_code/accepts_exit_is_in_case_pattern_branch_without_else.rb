def something
  array.each do |item|
    case cond
    in 1
      something
      exit
    in 2
      something2
      exit
    end
    bar
  end
end
