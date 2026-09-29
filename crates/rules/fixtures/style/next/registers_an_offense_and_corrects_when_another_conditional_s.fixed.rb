[].each do
  if foo?
    work
  end

  next unless bar?
  work
end
