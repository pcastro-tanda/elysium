try = 0
while try < max_tries
  try += 1
  next if weak?
  try = 0
end

raise(CombinationPoolExhaustedError)
