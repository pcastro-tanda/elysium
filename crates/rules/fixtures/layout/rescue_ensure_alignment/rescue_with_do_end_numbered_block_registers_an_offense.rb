def foo
  [1, 2, 3].each do
    _1.to_s
rescue StandardError => _exception
^^^^^^ `rescue` at 4, 0 is not aligned with `[1, 2, 3].each do` at 2, 2.
    next
  end
end
