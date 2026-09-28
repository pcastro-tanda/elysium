@instance = [1, 2, 3].map do |el|
  rescue StandardError => _exception
  ^^^^^^ `rescue` at 2, 2 is not aligned with `@instance` at 1, 0.
end
