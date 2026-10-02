foo.reduce([]) do |acc, el|
  break if something?
  acc << el
end
