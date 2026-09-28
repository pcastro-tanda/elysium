def something
  array.each do |item|
    begin
      abort
      bar
      ^^^ Unreachable code detected.
    end
  end
end
