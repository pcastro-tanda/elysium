module A
  protected
  [1, 2].map do |i|
    define_method("method#{i}") do
      i
    end
  end
end
