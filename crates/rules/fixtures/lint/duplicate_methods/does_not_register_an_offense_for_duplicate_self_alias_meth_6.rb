A = Class.new do
  alias_method :some_method, unknown()
  def some_method
    implement 1
  end
end
