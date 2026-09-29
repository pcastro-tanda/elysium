foo.each do |v|
  if v
    bar { |v| v&.baz }
  end
end
