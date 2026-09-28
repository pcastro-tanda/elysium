def something
  array.each do |item|
    begin
      next
      bar
      ^^^ Unreachable code detected.
    end
  end
end
