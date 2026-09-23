A = Class.new do
  def some_method
    implement 1
  end

  delegate :method, prefix: some_condition, to: :some
end
