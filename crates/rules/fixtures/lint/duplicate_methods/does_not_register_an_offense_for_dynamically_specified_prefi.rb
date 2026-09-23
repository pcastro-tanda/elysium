class A
  def some_method
    implement 1
  end

  delegate :method, prefix: some_condition, to: :some
end
