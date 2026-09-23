class A
  def some_method
    implement 1
  end
  alias_method :some_method, :any_method
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Method `A#some_method` is defined at both example.rb:2 and example.rb:5.
end
