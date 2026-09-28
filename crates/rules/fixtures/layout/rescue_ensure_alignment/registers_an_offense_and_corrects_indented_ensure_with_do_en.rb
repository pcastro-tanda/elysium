[1, 2, 3]
  .each do |el|
    el.to_s
      ensure
      ^^^^^^ `ensure` at 4, 6 is not aligned with `.each do` at 2, 2.
    next
  end
