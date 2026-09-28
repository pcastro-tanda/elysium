def something
  array.each do |item|
    begin
      raise
      bar
      ^^^ Unreachable code detected.
    end
  end
end
