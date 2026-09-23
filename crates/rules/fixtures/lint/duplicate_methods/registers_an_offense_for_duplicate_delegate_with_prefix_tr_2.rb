module A
  def some_method
    implement 1
  end
  delegate :method, to: :some, prefix: true
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Method `A#some_method` is defined at both example.rb:2 and example.rb:5.
end
