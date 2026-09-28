def something
  array.each do |item|
    def self.fail; end
    fail
    bar
  end
end
