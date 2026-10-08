array
  .map(&:to_s)
   ^^^ Remove redundant `map(&:to_s)` before `join`.
  .join(', ')
