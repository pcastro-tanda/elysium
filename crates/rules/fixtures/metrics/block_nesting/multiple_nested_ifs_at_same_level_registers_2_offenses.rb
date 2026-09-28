if a
  if b
    if c
    ^^^^ Avoid more than 2 levels of block nesting.
      puts c
    end
  end
  if d
    if e
    ^^^^ Avoid more than 2 levels of block nesting.
      puts e
    end
  end
end
