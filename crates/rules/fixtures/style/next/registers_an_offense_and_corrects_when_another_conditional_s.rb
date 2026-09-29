[].each do
  if foo?
    work
  end

  if bar?
  ^^^^^^^ Use `next` to skip iteration.
    work
  end
end
