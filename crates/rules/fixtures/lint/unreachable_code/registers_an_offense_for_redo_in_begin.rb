def something
  array.each do |item|
    begin
      redo
      bar
      ^^^ Unreachable code detected.
    end
  end
end
