a += [].map do |_|
  rescue StandardError => _
  ^^^^^^ `rescue` at 2, 2 is not aligned with `a` at 1, 0.
end
