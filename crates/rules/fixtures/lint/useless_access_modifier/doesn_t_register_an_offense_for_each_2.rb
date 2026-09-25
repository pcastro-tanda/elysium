module A
  private
  [1, 2].each do |i|
    define_method("method#{i}") do
      i
    end
  end
end
