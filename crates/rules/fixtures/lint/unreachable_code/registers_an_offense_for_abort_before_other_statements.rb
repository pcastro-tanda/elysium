def something
  array.each do |item|
    abort
    bar
    ^^^ Unreachable code detected.
  end
end
