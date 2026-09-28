def something
  array.each do |item|
    def abort; end
    abort
    bar
  end
end
