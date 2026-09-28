if a
  if b
    begin
      puts c
    rescue
    ^^^^^^ Avoid more than 2 levels of block nesting.
      puts x
    end
  end
end
