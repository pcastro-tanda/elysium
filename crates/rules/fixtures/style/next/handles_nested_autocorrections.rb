loop do
  if test
  ^^^^^^^ Use `next` to skip iteration.
    loop do
      if test
      ^^^^^^^ Use `next` to skip iteration.
        something
      end
    end
  end
end
