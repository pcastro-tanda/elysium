def something
  array.each do |item|
    def fail
      fail
      bar
    end
    
    fail
    bar
  end
end
