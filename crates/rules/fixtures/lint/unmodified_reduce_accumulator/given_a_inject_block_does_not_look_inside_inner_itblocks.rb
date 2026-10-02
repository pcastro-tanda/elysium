foo.inject(bar) do |acc, el|
  values.map do
    next el if something?
    it
  end
end
