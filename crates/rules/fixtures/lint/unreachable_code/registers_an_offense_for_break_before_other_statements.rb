def something
  array.each do |item|
    break
    bar
    ^^^ Unreachable code detected.
  end
end
