def something
  array.each do |item|
    return
    bar
    ^^^ Unreachable code detected.
  end
end
