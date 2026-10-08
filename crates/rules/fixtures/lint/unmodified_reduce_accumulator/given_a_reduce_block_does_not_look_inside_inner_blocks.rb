foo.reduce(bar) do |acc, el|
  values.map do |v|
    next el if something?
    el
  end
end
