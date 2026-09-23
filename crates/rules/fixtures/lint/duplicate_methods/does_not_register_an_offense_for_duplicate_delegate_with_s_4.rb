A = Module.new do
  def some_method
    implement 1
  end

  delegate :some_method, **options
end
