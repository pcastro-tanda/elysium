def foo
  [1, 2, 3].each do
    it.to_s
  rescue StandardError => _exception
    next
  end
end
