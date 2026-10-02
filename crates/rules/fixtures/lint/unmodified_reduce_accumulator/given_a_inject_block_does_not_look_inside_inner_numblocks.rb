foo.inject(bar) do |acc, el|
  values.map do
    next el if something?
    _1
  end
end
