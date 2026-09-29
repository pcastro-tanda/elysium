foo.each do |v|
  if v
    class << self
      v = bar
      v&.baz
    end
  end
end
