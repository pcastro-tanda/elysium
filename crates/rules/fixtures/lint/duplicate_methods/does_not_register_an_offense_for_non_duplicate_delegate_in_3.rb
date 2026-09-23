A.class_eval do
  def some_method
    implement 1
  end
  delegate :other_method, to: :foo
end
