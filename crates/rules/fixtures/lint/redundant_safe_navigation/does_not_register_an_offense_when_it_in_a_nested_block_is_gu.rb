foo.each do
  if it
    bar { it&.baz }
  end
end
