foo.each do |v|
  if v
    def m
      v = bar
      v&.baz
    end
  end
end
