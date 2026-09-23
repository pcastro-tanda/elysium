A.class_eval do
  def some_method
    implement 1
  end

  %w[any none some].each do |type|
    delegate :method, prefix: true, to: type
  end
end
