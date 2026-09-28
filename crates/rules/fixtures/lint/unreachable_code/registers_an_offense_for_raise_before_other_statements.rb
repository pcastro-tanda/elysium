def something
  array.each do |item|
    raise
    bar
    ^^^ Unreachable code detected.
  end
end
