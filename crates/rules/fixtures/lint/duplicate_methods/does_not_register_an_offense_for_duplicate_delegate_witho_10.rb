A.class_eval do
  def some_method
    implement 1
  end

  delegate :some_method, prefix: true
end
