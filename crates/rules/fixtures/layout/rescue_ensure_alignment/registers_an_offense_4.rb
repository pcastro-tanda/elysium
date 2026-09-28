$global = [].map do |_|
  rescue StandardError => _
  ^^^^^^ `rescue` at 2, 2 is not aligned with `$global` at 1, 0.
end
