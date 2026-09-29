foo.each do |v|
  if v
    class C
      v = bar
      v&.baz
    end
  end
end
