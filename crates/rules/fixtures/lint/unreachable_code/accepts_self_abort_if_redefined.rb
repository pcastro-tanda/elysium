def something
  array.each do |item|
    def self.abort; end
    abort
    bar
  end
end
