def something
  array.each do |item|
    redo
    bar
    ^^^ Unreachable code detected.
  end
end
