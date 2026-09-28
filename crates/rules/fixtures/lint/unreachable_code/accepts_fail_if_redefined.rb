def something
  array.each do |item|
    def fail; end
    fail
    bar
  end
end
