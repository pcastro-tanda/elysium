class A
  def some_method
    implement 1
  end
  delegate :method, prefix: false, to: :some
end
