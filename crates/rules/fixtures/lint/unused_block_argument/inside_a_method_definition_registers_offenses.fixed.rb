test do |_key, _value|
  def other(a)
    puts something(binding)
  end
end
