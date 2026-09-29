foo.each do |v|
  if v
    def self.m
      v = bar
      v&.baz
    end
  end
end
