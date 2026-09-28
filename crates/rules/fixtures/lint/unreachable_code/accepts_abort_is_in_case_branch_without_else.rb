def something
  array.each do |item|
    case cond
    when 1
      something
      abort
    when 2
      something2
      abort
    end
    bar
  end
end
