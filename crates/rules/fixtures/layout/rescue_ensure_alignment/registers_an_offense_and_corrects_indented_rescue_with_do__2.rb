[1, 2, 3].
  each do |el|
    el.to_s
      rescue StandardError => _exception
      ^^^^^^ `rescue` at 4, 6 is not aligned with `each do` at 2, 2.
    next
  end
